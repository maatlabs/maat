mod geometry;
mod math;

use geometry::Point;
use math::add;

let p = Point { x: 3, y: 4 };
let total = add(p.x, p.y);
print(total);
print(p.sum());
