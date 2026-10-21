fn add(a: i32, b: i32) -> i32 {
    a + b
}
fn main() { assert_eq!(add(2,3),5); assert_eq!(add(-7,5),-2); assert_eq!(add(0,0),0); assert_eq!(add(i32::MAX,0),i32::MAX); }
