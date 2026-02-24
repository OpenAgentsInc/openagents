use dsrs::{Optimizable, Predict, sign};
use rstest::*;

#[derive(Optimizable)]
struct Leaf {
    #[parameter]
    predictor: Predict,
}

#[derive(Optimizable)]
struct Parent {
    #[parameter]
    a: Predict,
    #[parameter]
    b: Leaf,
}

#[derive(Optimizable)]
struct GrandParent {
    #[parameter]
    p: Parent,
    #[parameter]
    c: Predict,
}

fn new_predict() -> Predict {
    Predict::new(sign! { (question: String) -> answer: String })
}

#[rstest]
fn test_flattens_two_levels_and_updates() {
    let mut parent = Parent {
        a: new_predict(),
        b: Leaf {
            predictor: new_predict(),
        },
    };

    // Check flattened names
    let mut names: Vec<String> = parent.parameters().keys().cloned().collect();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "b.predictor".to_string()]);

    // Update all signatures via returned params
    for (name, param) in parent.parameters() {
        param
            .update_signature_instruction(format!("X {name}"))
            .unwrap();
    }

    assert_eq!(parent.a.signature.instruction(), "X a");
    assert_eq!(parent.b.predictor.signature.instruction(), "X b.predictor");
}

#[rstest]
fn test_flattens_three_levels_and_updates() {
    let mut grand = GrandParent {
        p: Parent {
            a: new_predict(),
            b: Leaf {
                predictor: new_predict(),
            },
        },
        c: new_predict(),
    };

    // Check flattened names
    let mut names: Vec<String> = grand.parameters().keys().cloned().collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "c".to_string(),
            "p.a".to_string(),
            "p.b.predictor".to_string(),
        ]
    );

    // Update all signatures via returned params
    for (name, param) in grand.parameters() {
        param
            .update_signature_instruction(format!("Y {name}"))
            .unwrap();
    }

    assert_eq!(grand.c.signature.instruction(), "Y c");
    assert_eq!(grand.p.a.signature.instruction(), "Y p.a");
    assert_eq!(
        grand.p.b.predictor.signature.instruction(),
        "Y p.b.predictor"
    );
}

#[rstest]
fn test_ordering_of_parameters() {
    let mut grand = GrandParent {
        p: Parent {
            a: new_predict(),
            b: Leaf {
                predictor: new_predict(),
            },
        },
        c: new_predict(),
    };

    for _ in 0..50 {
        let names: Vec<String> = grand.parameters().keys().cloned().collect();
        let order = ["p.a", "p.b.predictor", "c"];

        for (name1, name2) in names.iter().zip(order.iter()) {
            assert_eq!(name1, name2);
        }
    }
}

#[rstest]
fn test_get_signature_and_update_instruction_for_single_parameter() {
    let mut leaf = Leaf {
        predictor: new_predict(),
    };
    let before = leaf.get_signature().instruction().to_string();

    leaf.update_signature_instruction("Leaf instruction".to_string())
        .unwrap();
    assert_ne!(leaf.get_signature().instruction(), before);
    assert_eq!(leaf.get_signature().instruction(), "Leaf instruction");
}

#[rstest]
fn test_multi_parameter_update_instruction_updates_all_children() {
    let mut parent = Parent {
        a: new_predict(),
        b: Leaf {
            predictor: new_predict(),
        },
    };
    let original_instruction = parent.get_signature().instruction().to_string();
    assert_eq!(original_instruction, parent.a.signature.instruction());

    parent
        .update_signature_instruction("Parent instruction".to_string())
        .unwrap();

    assert_eq!(parent.a.signature.instruction(), "Parent instruction");
    assert_eq!(
        parent.b.predictor.signature.instruction(),
        "Parent instruction"
    );
}
