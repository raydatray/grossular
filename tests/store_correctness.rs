use std::collections::HashMap;

use grossular::Store;

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn run_trace(bucket_count: usize) {
    let mut store = Store::new(bucket_count);
    let mut expected = HashMap::<Vec<u8>, Vec<u8>>::new();
    let mut random = 0x4d59_5df4_d0f3_3173;

    for _ in 0..10_000 {
        let key = (next_random(&mut random) % 256).to_le_bytes();

        if next_random(&mut random).is_multiple_of(4) {
            assert_eq!(
                store.get(&key),
                expected.get(key.as_slice()).map(Vec::as_slice)
            );
        } else {
            let value = next_random(&mut random).to_le_bytes();

            store.upsert(&key, &value);
            expected.insert(key.to_vec(), value.to_vec());
        }
    }

    for (key, expected_value) in &expected {
        assert_eq!(store.get(key), Some(expected_value.as_slice()));
    }
}

#[test]
fn matches_hashmap_with_one_bucket() {
    run_trace(1);
}

#[test]
fn matches_hashmap_with_sixteen_buckets() {
    run_trace(16);
}

#[test]
fn matches_hashmap_with_1024_buckets() {
    run_trace(1024);
}
