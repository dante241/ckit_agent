use evalfix::add;

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
    assert_eq!(add(10, 5), 15);
}

#[test]
fn adds_negatives_and_zero() {
    assert_eq!(add(-3, 7), 4);
    assert_eq!(add(0, 0), 0);
    assert_eq!(add(-4, -6), -10);
}
