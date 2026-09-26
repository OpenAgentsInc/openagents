//! Round-trip a fixed public marker in the probe's own Keychain namespace.
//!
//! This does not test locked-device access, migration, or a production signer.

use objc2::{class, msg_send, runtime::AnyObject};
use objc2_foundation::NSString;
use std::ffi::c_void;

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecClass: *const AnyObject;
    static kSecClassGenericPassword: *const AnyObject;
    static kSecAttrAccount: *const AnyObject;
    static kSecAttrService: *const AnyObject;
    static kSecAttrAccessible: *const AnyObject;
    static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: *const AnyObject;
    static kSecValueData: *const AnyObject;
    static kSecReturnData: *const AnyObject;
    fn SecItemAdd(query: *const AnyObject, result: *mut *mut AnyObject) -> i32;
    fn SecItemCopyMatching(query: *const AnyObject, result: *mut *mut AnyObject) -> i32;
    fn SecItemDelete(query: *const AnyObject) -> i32;
}

/// Return API status codes and byte equality without disclosing stored bytes.
pub(super) fn probe() -> serde_json::Value {
    // SAFETY: These dictionary keys are Security framework constants. All
    // Objective-C objects are retained until the synchronous calls return.
    // The account and service name identify only this public fixture.
    unsafe {
        let query: *mut AnyObject = msg_send![class!(NSMutableDictionary), new];
        let account = NSString::from_str("synthetic-only-v1");
        let service = NSString::from_str("com.openagents.coder.platformprobe");
        let _: () = msg_send![query, setObject: kSecClassGenericPassword forKey: kSecClass];
        let _: () = msg_send![query, setObject: &*account forKey: kSecAttrAccount];
        let _: () = msg_send![query, setObject: &*service forKey: kSecAttrService];
        // Remove only this app's fixed synthetic item from an earlier run.
        let initial_delete = SecItemDelete(query);
        let marker = b"public-synthetic-keychain-marker";
        let data: *mut AnyObject = msg_send![class!(NSData), dataWithBytes: marker.as_ptr().cast::<c_void>() length: marker.len()];
        let _: () = msg_send![query, setObject: data forKey: kSecValueData];
        let _: () = msg_send![query, setObject: kSecAttrAccessibleWhenUnlockedThisDeviceOnly forKey: kSecAttrAccessible];
        let add = SecItemAdd(query, std::ptr::null_mut());
        let _: () = msg_send![query, removeObjectForKey: kSecValueData];
        let _: () = msg_send![query, removeObjectForKey: kSecAttrAccessible];
        let yes: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: true];
        let _: () = msg_send![query, setObject: yes forKey: kSecReturnData];
        let mut returned: *mut AnyObject = std::ptr::null_mut();
        let read = SecItemCopyMatching(query, &mut returned);
        let equal = if read == 0 && !returned.is_null() {
            let length: usize = msg_send![returned, length];
            let bytes: *const c_void = msg_send![returned, bytes];
            let equal = length == marker.len()
                && !bytes.is_null()
                && std::slice::from_raw_parts(bytes.cast::<u8>(), length) == marker;
            let _: () = msg_send![returned, release];
            equal
        } else {
            false
        };
        let _: () = msg_send![query, removeObjectForKey: kSecReturnData];
        let delete = SecItemDelete(query);
        let absent = SecItemCopyMatching(query, std::ptr::null_mut());
        let _: () = msg_send![query, release];
        serde_json::json!({
            "event":"keychain_probe", "synthetic":true,
            "initial_delete_status":initial_delete, "add_status":add,
            "read_status":read, "bytes_equal":equal,
            "delete_status":delete, "read_after_delete_status":absent,
            "passed":add == 0 && read == 0 && equal && delete == 0 && absent == -25300,
            "locked_device_test":"not_run", "at_ms":atif::now_ms()
        })
    }
}
