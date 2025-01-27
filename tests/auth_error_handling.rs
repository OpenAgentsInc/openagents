#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;
    use wasm_bindgen::JsValue;
    use web_sys::{Document, Element, HtmlElement};

    wasm_bindgen_test_configure!(run_in_browser);

    fn setup_test_dom() -> Document {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        
        // Create and add error container
        let error_div = document.create_element("div").unwrap();
        error_div.set_id("auth-error");
        error_div.set_class_name("hidden");
        
        let error_message = document.create_element("span").unwrap();
        error_message.set_id("auth-error-message");
        error_div.append_child(&error_message).unwrap();
        
        document.body().unwrap().append_child(&error_div).unwrap();
        
        document
    }

    #[wasm_bindgen_test]
    fn test_show_auth_error() {
        let document = setup_test_dom();
        
        // Call showAuthError
        js_sys::eval("showAuthError('Test error message')").unwrap();
        
        // Check if error is displayed
        let error_div = document.get_element_by_id("auth-error").unwrap();
        let error_message = document.get_element_by_id("auth-error-message").unwrap();
        
        assert!(!error_div.class_list().contains("hidden"));
        assert_eq!(error_message.text_content().unwrap(), "Test error message");
    }

    #[wasm_bindgen_test]
    fn test_clear_auth_error() {
        let document = setup_test_dom();
        
        // Show error first
        js_sys::eval("showAuthError('Test error message')").unwrap();
        
        // Clear error
        js_sys::eval("clearAuthError()").unwrap();
        
        // Check if error is hidden
        let error_div = document.get_element_by_id("auth-error").unwrap();
        assert!(error_div.class_list().contains("hidden"));
    }

    #[wasm_bindgen_test]
    fn test_handle_known_error() {
        let document = setup_test_dom();
        
        // Test with known error code
        js_sys::eval("handleAuthError('invalid_credentials')").unwrap();
        
        let error_message = document.get_element_by_id("auth-error-message").unwrap();
        assert_eq!(error_message.text_content().unwrap(), "Invalid email or password.");
    }

    #[wasm_bindgen_test]
    fn test_handle_unknown_error() {
        let document = setup_test_dom();
        
        // Test with unknown error message
        js_sys::eval("handleAuthError('some_unknown_error')").unwrap();
        
        let error_message = document.get_element_by_id("auth-error-message").unwrap();
        assert_eq!(error_message.text_content().unwrap(), "some_unknown_error");
    }
}