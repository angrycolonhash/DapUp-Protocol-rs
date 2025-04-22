use esp_idf_sys::xTaskCreatePinnedToCore;
use std::{ffi::{c_void, CString}, ptr};


/// Example of code: 
/// ```rust
/// unsafe extern "C" fn advertise_thread(arg: *mut c_void) {
/// }
/// ```
pub unsafe fn thread_start(name_of_task: &str, function: unsafe extern "C" fn(*mut c_void)) {
    let task_name = CString::new(name_of_task).unwrap();
    let mut task_handle = ptr::null_mut();
    
    let result = xTaskCreatePinnedToCore(
        Some(function),
        task_name.as_ptr(),
        4096,                 // Stack size in words
        ptr::null_mut(),      // Parameters to pass to the task
        1,                    // Priority (1 is low)
        &mut task_handle,     // Task handle
        0,                    // Core ID (0)
    );

    if result != 1 {
        log::warn!("Failed to create {}: {}", name_of_task, result);
    } else {
        log::info!("{} task created successfully", name_of_task);
    }
}