use serde_json::json;

pub fn get_file_list_test_response() -> String {
    json!([
        "src/lib.rs",
        "src/main.rs"
    ]).to_string()
}

pub fn get_change_generation_test_response() -> String {
    r#"src/lib.rs
<<<<<<< SEARCH
pub fn add(a: i32, b: i32) -> i32 { a + b }
=======
pub fn add(a: i32, b: i32) -> i32 { a + b }
pub fn multiply(a: i32, b: i32) -> i32 { a * b }
>>>>>>> REPLACE"#.to_string()
}