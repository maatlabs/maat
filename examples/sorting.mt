let insert_sorted = fn(sorted, val) {
    let build = fn(remaining, acc, inserted) {
        if (len(remaining) == 0) {
            if (inserted) { acc } else { push(acc, val) }
        } else {
            let head = first(remaining);
            let tail = rest(remaining);
            if (!inserted) {
                if (val < head) {
                    build(tail, push(push(acc, val), head), true)
                } else {
                    build(tail, push(acc, head), false)
                }
            } else {
                build(tail, push(acc, head), true)
            }
        }
    };
    build(sorted, [], false);
};

let insertion_sort = fn(arr) {
    let iter = fn(arr, acc) {
        if (len(arr) == 0) {
            acc
        } else {
            iter(rest(arr), insert_sorted(acc, first(arr)));
        }
    };
    iter(arr, []);
};

let data = [64, 34, 25, 12, 22, 11, 90, 1, 45, 78];
print(insertion_sort(data));

let reverse = fn(arr) {
    let build = fn(i: i64, acc) {
        if (i < 0) {
            acc
        } else {
            build(i - 1, push(acc, arr[i]));
        }
    };
    build(len(arr) as i64 - 1, []);
};

print(reverse([1, 2, 3, 4, 5]));
