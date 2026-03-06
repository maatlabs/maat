struct Point { x: i64, y: i64 }

impl Point {
    fn sum(self) -> i64 { self.x + self.y }
}

let p = Point { x: 3, y: 4 };
print(p.x);
print(p.y);
print(p.sum());

enum Shape { Circle(i64), Rect(i64, i64) }

let area = fn(s: Shape) -> i64 {
    match s { Circle(r) => r * r, Rect(w, h) => w * h }
};

print(area(Shape::Circle(5)));
print(area(Shape::Rect(3, 4)));

fn safe_div(a: i64, b: i64) -> Option<i64> {
    if (b == 0) { Option::None } else { Option::Some(a / b) }
}

let r1 = safe_div(10, 2);
let r2 = safe_div(10, 0);
print(match r1 { Some(v) => v, None => -1 });
print(match r2 { Some(v) => v, None => -1 });

fn checked_div(a: i64, b: i64) -> Result<i64, i64> {
    if (b == 0) { Result::Err(-1) } else { Result::Ok(a / b) }
}

let r3 = checked_div(20, 5);
let r4 = checked_div(20, 0);
print(match r3 { Ok(v) => v, Err(e) => e });
print(match r4 { Ok(v) => v, Err(e) => e });
