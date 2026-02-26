let make_adder = fn(x: i64) {
    fn(y: i64) -> i64 { x + y; };
};

let add5 = make_adder(5);
let add10 = make_adder(10);

print(add5(3));
print(add10(3));
print(add5(add10(1)));
