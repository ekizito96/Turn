use turn::value::Value;

#[tokio::test]
async fn test_vector_creation() {
    let source = "let v = vec[1, 2, 3]; return v;";
    let val = turn::run(source).unwrap();
    if let Value::Vec(v) = val {
        assert_eq!(*v, vec![1.0, 2.0, 3.0]);
    }
}
#[tokio::test]
async fn test_vector_addition() {
    let source = "let v1 = vec[1, 2]; let v2 = vec[3, 4]; return v1 + v2;";
    let val = turn::run(source).unwrap();
    if let Value::Vec(v) = val {
        assert_eq!(*v, vec![4.0, 6.0]);
    }
}
