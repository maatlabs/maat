let binary_search = fn(arr, target) {
    let search = fn(low, high) {
        if (low > high) {
            -1
        } else {
            let mid = low + (high - low) / 2;
            let val = arr[mid];
            if (val == target) {
                mid
            } else {
                if (val < target) {
                    search(mid + 1, high);
                } else {
                    search(low, mid - 1);
                }
            }
        }
    };
    search(0, len(arr) as i64 - 1);
};

let sorted = [2, 5, 8, 12, 16, 23, 38, 56, 72, 91];

print(binary_search(sorted, 23));
print(binary_search(sorted, 100));
