use std::fs;

fn main() {
    let source = fs::read_to_string("tests/test_actor.turn").unwrap();
    println!("Char at 143: {:?}", &source[143..144]);
    println!("Context: {:?}", &source[130..150]);
}
