#[tokio::test]
async fn test_generic_list() {
    let source = "let l: List<Num> = [1, 2, 3]; return l;";
    let _val = turn::run(source).unwrap();
}
