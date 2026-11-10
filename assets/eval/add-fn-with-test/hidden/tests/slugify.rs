use evalslug::slugify;

#[test]
fn slug() {
    assert_eq!(slugify("Hello World"), "hello-world");
    assert_eq!(slugify("A B!c"), "a-bc");
    assert_eq!(slugify("rust_LANG 2026"), "rustlang-2026");
}

#[test]
fn each_space_and_non_ascii() {
    assert_eq!(slugify("Hello  World"), "hello--world");
    assert_eq!(slugify("ÄBC 1"), "bc-1");
}
