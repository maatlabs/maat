let insert_sorted = fn(sorted, val) {
    let build = fn(remaining, acc, inserted) {
        if (remaining.len() == 0usize) {
            if (inserted) { acc } else { acc.push(val) }
        } else {
            let head = remaining.first();
            let tail = remaining.rest();
            if (!inserted) {
                if (val < head) {
                    build(tail, acc.push(val).push(head), true)
                } else {
                    build(tail, acc.push(head), false)
                }
            } else {
                build(tail, acc.push(head), true)
            }
        }
    };
    build(sorted, [], false);
};

let insertion_sort = fn(arr) {
    let iter = fn(arr, acc) {
        if (arr.len() == 0usize) {
            acc
        } else {
            iter(arr.rest(), insert_sorted(acc, arr.first()));
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
            build(i - 1, acc.push(arr[i]));
        }
    };
    build(arr.len() as i64 - 1, []);
};

print(reverse([1, 2, 3, 4, 5]));
