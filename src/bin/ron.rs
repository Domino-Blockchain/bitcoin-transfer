use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct MyStruct {
    boolean: bool,
    float: f32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Data {
    request: Option<i32>,
    code: i32,
}

fn main() {
    let data = include_str!("test.ron");
    let d: ron::value::Value = match ron::from_str(data) {
        Ok(d) => d,
        Err(err) => {
            let line = data.lines().nth(err.position.line - 1).unwrap();
            dbg!(line);
            let start = err.position.col.saturating_sub(10);
            let end = (err.position.col + 10).min(line.len());
            dbg!(line.get(start..end).unwrap());
            panic!("{err:?}");
        }
    };
    println!(
        "Pretty RON: {}",
        ron::ser::to_string_pretty(&d, ron::ser::PrettyConfig::default()).unwrap(),
    );

    let d: Data = match ron::from_str(data) {
        Ok(d) => d,
        Err(err) => {
            let line = data.lines().nth(err.position.line - 1).unwrap();
            dbg!(line);
            let start = err.position.col.saturating_sub(10);
            let end = (err.position.col + 10).min(line.len());
            dbg!(line.get(start..end).unwrap());
            panic!("{err:?}");
        }
    };
    println!(
        "Pretty RON: {}",
        ron::ser::to_string_pretty(&d, ron::ser::PrettyConfig::default()).unwrap(),
    );

    // let x: MyStruct = ron::from_str("(boolean: true, float: 1.23)").unwrap();

    // println!("RON: {}", ron::to_string(&x).unwrap());

    // println!(
    //     "Pretty RON: {}",
    //     ron::ser::to_string_pretty(&x, ron::ser::PrettyConfig::default()).unwrap(),
    // );
}
