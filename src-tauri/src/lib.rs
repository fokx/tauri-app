use futures::{future, StreamExt};
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use tauri::{Emitter, Manager};
use tokio;

use sysinfo::{Disks, System};

// #[cfg(any(target_os = "android", target_os = "ios"))]
// use test::Foo;

// #[cfg(not(any(target_os = "android", target_os = "ios")))] // desktop
// use tauri_plugin_shell::ShellExt;

use env_logger::Builder as LoggerBuilder;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{env, process};

use crate::{
    config::{Config, ConfigError},
    connection::Connection,
    socks5::Server as Socks5Server,
};

use reqwest::Method;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

mod config;
mod connection;
mod error;
mod socks5;
mod utils;

const REQ_HOST: &str = "xjtu.app";
const REQ_PORT: u16 = 443;
// use futures::executor::block_on;
#[tauri::command]
fn collect_nic_info() -> String {
    let network_interfaces = NetworkInterface::show().unwrap();
    let mut result: String = "".to_owned();
    for itf in network_interfaces.iter() {
        result.push_str(&format!("{:?}", itf));
    }
    let mut sys = System::new_all();
    sys.refresh_all();
    result.push_str(&format!("=> system:"));
    // RAM and swap information:
    result.push_str(&format!("total memory: {} bytes", sys.total_memory()));
    result.push_str(&format!("used memory : {} bytes", sys.used_memory()));
    result.push_str(&format!("total swap  : {} bytes", sys.total_swap()));
    result.push_str(&format!("used swap   : {} bytes", sys.used_swap()));

    // Display system information:
    result.push_str(&format!("System name:             {:?}", System::name()));
    result.push_str(&format!(
        "System kernel version:   {:?}",
        System::kernel_version()
    ));
    result.push_str(&format!(
        "System OS version:       {:?}",
        System::os_version()
    ));
    result.push_str(&format!(
        "System host name:        {:?}",
        System::host_name()
    ));

    // Number of CPUs:
    result.push_str(&format!("NB CPUs: {}", sys.cpus().len()));

    // Display processes ID, name na disk usage:
    for (pid, process) in sys.processes() {
        result.push_str(&format!(
            "[{pid}] {:?} {:?}",
            process.name(),
            process.disk_usage()
        ));
    }

    // We display all disks' information:
    result.push_str(&format!("=> disks:"));
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        result.push_str(&format!("{disk:?}"));
    }

    return result;
}
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
#[tauri::command]
fn collect_center_server_ip() -> String {
    const CENTRAL_SERVER_IP_SRC_URLS: &'static [&'static str] = &[
        "https://raw.githubusercontent.com/xjtu-men/domains/main/xjtu.men.server.ip",
        "https://gitea.com/xjtu-men/domains/raw/branch/main/xjtu.men.server.ip",
    ];

    let client = reqwest::Client::new();
    let bodies = future::join_all(CENTRAL_SERVER_IP_SRC_URLS.into_iter().map(|url| {
        let client = &client;
        async move {
            let resp = client.get(*url).send().await?;
            resp.bytes().await
        }
    }));
    let mut result: String = "".to_owned();
    for body in tauri::async_runtime::block_on(bodies) {
        result.push_str(&format!("{:?}", body));
    }
    return result;
}

async fn tcc_main() {
    let cfg = match Config::parse(env::args_os()) {
        Ok(cfg) => cfg,
        Err(ConfigError::Version(msg) | ConfigError::Help(msg)) => {
            println!("{msg}");
            process::exit(0);
        }
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    };

    LoggerBuilder::new()
        .filter_level(cfg.log_level)
        .format_module_path(false)
        .format_target(false)
        .init();
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    match Connection::set_config(cfg.relay) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }

    match Socks5Server::set_config(cfg.local) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }

    Socks5Server::start().await;
}

async fn socks2http() {
    let listener = TcpListener::bind(format!("127.0.0.1:{:?}", 4802))
        .await
        .unwrap();
    let socks5_url = reqwest::Url::parse(
        &*format!("socks5h://127.0.0.1:{:?}", 4801).to_string(),
    )
            .unwrap();
    let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all(socks5_url).unwrap())
            .cookie_store(true)
            .use_rustls_tls()
            .tls_sni(true)
            .tls_info(true)
            .build()
            .unwrap();
    loop {
        let client = client.clone();
        let (mut inbound, addr) = listener.accept().await.unwrap();
        println!("NEW CLIENT: {}", addr);

        tokio::spawn(async move {
            let mut buf = [0; 1024 * 8];
            let Ok(downstream_read_bytes_size) = inbound.read(&mut buf).await else {
                return;
            };
            let bytes_from_downstream = &buf[0..downstream_read_bytes_size];

            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut req = httparse::Request::new(&mut headers);
            let Ok(parse_result) = req.parse(bytes_from_downstream) else {
                return;
            };
            if parse_result.is_complete() {
                if let Some(valid_req_path) = req.path {
                    println!("get request: {}", valid_req_path);

                    let outbound = TcpStream::connect(format!("127.0.0.1:{:?}", 4801))
                        .await
                        .unwrap();
                    println!("forwarding to socks5 proxy at port {}", 4801);
                    let mut outbound = io::BufStream::new(outbound);
                    async_socks5::connect(&mut outbound, (REQ_HOST, REQ_PORT), None)
                        .await
                        .unwrap();
                    println!("proxy server connected to {}", REQ_HOST);
                    dbg!(req.method.unwrap());
                    if req.method.unwrap() == Method::CONNECT { // HTTPS proxy use CONNECT command
                        inbound
                            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                            .await
                            .unwrap();
                        let (mut ri, mut wi) = inbound.split();
                        let (mut ro, mut wo) = outbound.get_mut().split();

                        let client_to_server = async {
                            io::copy(&mut ri, &mut wo)
                                .await
                                .expect("Transport endpoint is not connected");
                            wo.shutdown().await
                        };

                        let server_to_client = async {
                            let _ = io::copy(&mut ro, &mut wi).await;
                            wi.shutdown().await
                        };
                        println!("try join");
                        let _ = futures::future::try_join(client_to_server, server_to_client).await;
                    } else {

                        let req_url = format!("https://{}{}", REQ_HOST, valid_req_path);
                        println!("reqwest client built with SOCKS5 to {}", req_url);
                        // let response = client.get(req_url).send().await.unwrap();
                        let response = client.request(Method::GET, req_url).send().await.unwrap();
                        // let response = client.request(Method::GET,"https://myip.xjtu.app").send().await.unwrap();
                        // dbg!(response.version());
                        // dbg!(response.text().await.unwrap());

                        // let headers = response.headers();
                        // let body_text =response.text().await.unwrap();

                        inbound
                                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                                .await
                                .unwrap();
                        let response_bytes = response.bytes().await.unwrap();
                        let _ = inbound.write(&response_bytes).await;
                        inbound.flush().await.unwrap();

                        // Ok(hyper::Response::new(hyper::Body::from(body_text)))

                        // // Method = GET ...
                        // let upstream_write_bytes_size =
                        //     outbound.write(bytes_from_downstream).await.unwrap();
                        // assert_eq!(upstream_write_bytes_size, downstream_read_bytes_size);
                        //
                        // let (mut ri, mut wi) = inbound.split();
                        // let (mut ro, mut wo) = outbound.get_mut().split();
                        //
                        // io::copy(&mut ro, &mut wi)
                        //     .await.expect("Transport endpoint is not connected");
                        // wi.shutdown().await;
                        // wo.shutdown().await;
                    }
                }
            }
        });
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // mobile
        tauri::Builder::default()
            // .plugin(tauri_plugin_barcode_scanner::init())
            // .plugin(tauri_plugin_biometric::init())
            // .plugin(tauri_plugin_nfc::init())
            // .plugin(tauri_plugin_notification::init())
            // .plugin(tauri_plugin_fs::init())
            // .plugin(tauri_plugin_sql::Builder::new().build())
            // .plugin(tauri_plugin_http::init())
            // .plugin(tauri_plugin_opener::init())
            .setup(|app| {
                // std::thread::spawn(move || block_on(tcc_main()));
                tauri::async_runtime::spawn(tcc_main());
                tauri::async_runtime::spawn(socks2http());
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![greet, collect_nic_info])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // desktop
        tauri::Builder::default()
            // .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![]) /* arbitrary number of args to pass to your app */))
            // .plugin(tauri_plugin_shell::init())
            // .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            // .plugin(tauri_plugin_notification::init())
            // .plugin(tauri_plugin_fs::init())
            // .plugin(tauri_plugin_sql::Builder::new().build())
            // .plugin(tauri_plugin_http::init())
            // .plugin(tauri_plugin_opener::init())
            .setup(|app| {
                /* this shell codeb will cause crash on Windows!
                let handle = app.handle().clone();
                let shell = handle.shell();
                let output = tauri::async_runtime::block_on(async move {
                    shell
                            .command("echo")
                            .args(["Hello from Rust!"])
                            .output()
                            .await
                            .unwrap()
                });
                if output.status.success() {
                    println!("Result: {:?}", String::from_utf8(output.stdout));
                } else {
                    println!("Exit with code: {}", output.status.code().unwrap());
                };
                 */

                // disable sidecar in favor of implement this in rust code
                // let sidecar_command = app.shell().sidecar("tcc-xapp-hhk").unwrap();
                // let (mut rx, mut _child) =
                //     sidecar_command.spawn().expect("Failed to spawn sidecar");

                // std::thread::spawn(move || block_on(tcc_main()));
                tauri::async_runtime::spawn(tcc_main());
                tauri::async_runtime::spawn(socks2http());

                // let window = app.get_window("main").unwrap();
                // // let _ = window.destroy();
                // let window = tauri::window::WindowBuilder::new(app, "webview").build()?;
                // // let title = Config::get().unwrap().title.unwrap_or("xmen app".to_string());
                // // window.set_title("交大門 Tauri App");

                // let window = app.get_webview_window("main").unwrap();
                // // window.open_devtools();
                // // let tauri_url = tauri::WebviewUrl::App("index.html".into());
                // // let url = Url::parse("https://myip.xjtu.app:443")?;
                // let url = Url::parse("https://xjtu.app:443")?;
                // let tauri_url = WebviewUrl::External(url);
                // let webview_window =
                //     tauri::WebviewWindowBuilder::new(app, "label", tauri_url)
                //             .proxy_url(Url::parse("socks5://127.0.0.1:4801")?)
                //             // .devtools(true)
                //             .build()?;
                // webview_window.open_devtools();

                // WebviewWindowBuilder::new(
                //     "webview window", WebviewUrl::External(url::Url::parse("https://myip.xjtu.app")?)),
                //         // .proxy_url(Url::parse("socks5://127.0.0.1:4801")?) // may cause white screen
                //         .build()?;

                // let webview = window.add_child( // Available on desktop and crate feature unstable only.
                //                                 webview_builder,
                //                                 tauri::LogicalPosition::new(0, 0),
                //                                 window.inner_size().unwrap(),
                // );

                // let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                // let menu = Menu::with_items(app, &[&quit_i])?;
                // let tray = TrayIconBuilder::new()
                //     .menu(&menu)
                //     .show_menu_on_left_click(true)
                //     .on_menu_event(|app, event| match event.id.as_ref() {
                //         "quit" => {
                //             println!("quit menu item was clicked");
                //             app.exit(0);
                //         }
                //         _ => {
                //             println!("menu item {:?} not handled", event.id);
                //         }
                //     })
                //     .build(app)?;

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![greet, collect_nic_info])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}
