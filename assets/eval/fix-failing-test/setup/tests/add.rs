use evalfix::add;

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
    assert_eq!(add(10, 5), 15);
}
