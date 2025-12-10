//! Arithmetic Units: Basic math operations
//!
//! Provides units for common arithmetic operations:
//! - Add, Subtract, Multiply, Divide
//! - Modulo, Negate
//! - Increment, Decrement

// Use the macros to generate basic arithmetic units
crate::binary_op_unit!(Add, +, "Adds two numbers: a + b");
crate::binary_op_unit!(Subtract, -, "Subtracts two numbers: a - b");
crate::binary_op_unit!(Multiply, *, "Multiplies two numbers: a * b");
crate::binary_op_unit!(Divide, /, "Divides two numbers: a / b");
crate::binary_op_unit!(Modulo, %, "Modulo of two numbers: a % b");

// Unary operations
crate::unary_math_unit!(Negate, |x: f64| -x, "Negates a number: -x");
crate::unary_math_unit!(Increment, |x: f64| x + 1.0, "Increments a number: x + 1");
crate::unary_math_unit!(Decrement, |x: f64| x - 1.0, "Decrements a number: x - 1");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Unit;

    #[test]
    fn test_add() {
        let mut add = Add::new();
        add.play();

        add.push_input("a", Box::new(2.0f64)).unwrap();
        add.push_input("b", Box::new(3.0f64)).unwrap();

        let result = add.take_output("result");
        assert!(result.is_some());
        let value = result.unwrap().downcast::<f64>().unwrap();
        assert_eq!(*value, 5.0);
    }

    #[test]
    fn test_subtract() {
        let mut sub = Subtract::new();
        sub.play();

        sub.push_input("a", Box::new(10.0f64)).unwrap();
        sub.push_input("b", Box::new(3.0f64)).unwrap();

        let result = sub.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, 7.0);
    }

    #[test]
    fn test_multiply() {
        let mut mul = Multiply::new();
        mul.play();

        mul.push_input("a", Box::new(4.0f64)).unwrap();
        mul.push_input("b", Box::new(5.0f64)).unwrap();

        let result = mul.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, 20.0);
    }

    #[test]
    fn test_divide() {
        let mut div = Divide::new();
        div.play();

        div.push_input("a", Box::new(20.0f64)).unwrap();
        div.push_input("b", Box::new(4.0f64)).unwrap();

        let result = div.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, 5.0);
    }

    #[test]
    fn test_negate() {
        let mut neg = Negate::new();
        neg.play();

        neg.push_input("x", Box::new(5.0f64)).unwrap();

        let result = neg.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, -5.0);
    }

    #[test]
    fn test_increment() {
        let mut inc = Increment::new();
        inc.play();

        inc.push_input("x", Box::new(10.0f64)).unwrap();

        let result = inc.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, 11.0);
    }

    #[test]
    fn test_decrement() {
        let mut dec = Decrement::new();
        dec.play();

        dec.push_input("x", Box::new(10.0f64)).unwrap();

        let result = dec.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, 9.0);
    }

    #[test]
    fn test_modulo() {
        let mut modulo = Modulo::new();
        modulo.play();

        modulo.push_input("a", Box::new(17.0f64)).unwrap();
        modulo.push_input("b", Box::new(5.0f64)).unwrap();

        let result = modulo.take_output("result").unwrap();
        let value = result.downcast::<f64>().unwrap();
        assert_eq!(*value, 2.0);
    }
}
