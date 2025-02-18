use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;

use sysinfo::{
    Components, Disks, Networks, System,
};

#[tauri::command]
fn collect_nic_info() ->String {
    let network_interfaces = NetworkInterface::show().unwrap();
    let mut result : String = "".to_owned();
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
    result.push_str(&format!("System kernel version:   {:?}", System::kernel_version()));
    result.push_str(&format!("System OS version:       {:?}", System::os_version()));
    result.push_str(&format!("System host name:        {:?}", System::host_name()));

    // Number of CPUs:
    result.push_str(&format!("NB CPUs: {}", sys.cpus().len()));

    // Display processes ID, name na disk usage:
    for (pid, process) in sys.processes() {
        result.push_str(&format!("[{pid}] {:?} {:?}", process.name(), process.disk_usage()));
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet, collect_nic_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
