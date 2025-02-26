use futures::{future, StreamExt};
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use tauri::{Emitter, Manager, WebviewUrl};
use tokio;

use tauri::WebviewWindowBuilder;

use sysinfo::{Components, Disks, Networks, System};

// #[cfg(any(target_os = "android", target_os = "ios"))]
// use test::Foo;

#[cfg(not(any(target_os = "android", target_os = "ios")))] // desktop
use tauri_plugin_shell::ShellExt;

#[cfg(not(any(target_os = "android", target_os = "ios")))] // mobile
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use url::Url;

use std::{env, process};
use env_logger::Builder as LoggerBuilder;

use crate::{
    config::{Config, ConfigError},
    connection::Connection,
    socks5::Server as Socks5Server,
};

mod config;
mod connection;
mod error;
mod socks5;
mod utils;

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
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install rustls crypto provider");
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // mobile
        tauri::Builder::default()
            .plugin(tauri_plugin_barcode_scanner::init())
            .plugin(tauri_plugin_biometric::init())
            .plugin(tauri_plugin_nfc::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_sql::Builder::new().build())
            .plugin(tauri_plugin_http::init())
            .plugin(tauri_plugin_opener::init())
            .setup(|app| {
                // std::thread::spawn(move || block_on(tcc_main()));
                tauri::async_runtime::spawn(tcc_main());
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
            .plugin(tauri_plugin_shell::init())
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_sql::Builder::new().build())
            .plugin(tauri_plugin_http::init())
            .plugin(tauri_plugin_opener::init())
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
                //             .proxy_url(Url::parse("socks5://127.0.0.1:4848")?)
                //             // .devtools(true)
                //             .build()?;
                // webview_window.open_devtools();

                // WebviewWindowBuilder::new(
                //     "webview window", WebviewUrl::External(url::Url::parse("https://myip.xjtu.app")?)),
                //         // .proxy_url(Url::parse("socks5://127.0.0.1:4848")?) // may cause white screen
                //         .build()?;

                // let webview = window.add_child( // Available on desktop and crate feature unstable only.
                //                                 webview_builder,
                //                                 tauri::LogicalPosition::new(0, 0),
                //                                 window.inner_size().unwrap(),
                // );

                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&quit_i])?;
                let tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => {
                            println!("quit menu item was clicked");
                            app.exit(0);
                        }
                        _ => {
                            println!("menu item {:?} not handled", event.id);
                        }
                    })
                    .build(app)?;

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![greet, collect_nic_info])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}
