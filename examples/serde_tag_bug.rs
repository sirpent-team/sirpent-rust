extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

#[derive(Serialize, Deserialize)]
#[serde(tag = "tag")]
pub enum A {
    A1 { field: B },
    A2,
}

#[derive(Serialize, Deserialize)]
pub enum B {
    B1,
    B2,
}

fn main() {
    // Construct an A containing a B.
    let a = A::A1 { field: B::B1 };

    // Successfully serialises to `{"tag":"a","field":"B1"}`.
    let json = serde_json::to_string(&a).unwrap();
    println!("{}", json);

    // That correct, serde_json-generated JSON errors in the decode with
    // `Syntax(Message("invalid type: string \"B1\", expected enum B"), 0, 0)`
    let a_new: A = serde_json::from_str(json.as_str()).unwrap();
    //println!("{:?}", a_new);
}
