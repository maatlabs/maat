let map = fn(arr, f) {
    let iter = fn(arr, acc) {
        if (len(arr) == 0) {
            acc
        } else {
            iter(rest(arr), push(acc, f(first(arr))));
        }
    };
    iter(arr, []);
};

let reduce = fn(arr, init, f) {
    let iter = fn(arr, acc) {
        if (len(arr) == 0) {
            acc
        } else {
            iter(rest(arr), f(acc, first(arr)));
        }
    };
    iter(arr, init);
};

let filter = fn(arr, predicate) {
    let iter = fn(arr, acc) {
        if (len(arr) == 0) {
            acc
        } else {
            let head = first(arr);
            let tail = rest(arr);
            if (predicate(head)) {
                iter(tail, push(acc, head));
            } else {
                iter(tail, acc);
            }
        }
    };
    iter(arr, []);
};

let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

let double = fn(x: i64) -> i64 { x * 2; };
let add = fn(a: i64, b: i64) -> i64 { a + b; };
let is_even = fn(x) { x / 2 * 2 == x; };

let doubled = map(numbers, double);
let evens = filter(numbers, is_even);
let sum = reduce(doubled, 0, add);

print(doubled);
print(evens);
print(sum);
