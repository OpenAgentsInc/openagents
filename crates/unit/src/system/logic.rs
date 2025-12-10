//! Logic Units: Boolean operations
//!
//! Provides units for common logic operations:
//! - And, Or
//! - Not (unary)

// Use the macros for binary logic operations
crate::logic_gate_unit!(And, &&, "Logical AND: a && b");
crate::logic_gate_unit!(Or, ||, "Logical OR: a || b");

// Note: Not, Xor, Nand, Nor would need custom implementations or new macros
// For now, we provide just the basic gate units

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Unit;

    #[test]
    fn test_and() {
        let mut and = And::new();
        and.play();

        and.push_input("a", Box::new(true)).unwrap();
        and.push_input("b", Box::new(true)).unwrap();
        let result = and.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        and.push_input("a", Box::new(true)).unwrap();
        and.push_input("b", Box::new(false)).unwrap();
        let result = and.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, false);
    }

    #[test]
    fn test_or() {
        let mut or = Or::new();
        or.play();

        or.push_input("a", Box::new(false)).unwrap();
        or.push_input("b", Box::new(false)).unwrap();
        let result = or.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, false);

        or.push_input("a", Box::new(true)).unwrap();
        or.push_input("b", Box::new(false)).unwrap();
        let result = or.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);
    }
}
