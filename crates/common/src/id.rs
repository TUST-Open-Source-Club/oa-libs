use uuid::Uuid;

/// 生成 UUIDv7：时间有序，索引友好。
pub fn new_id() -> Uuid {
    Uuid::now_v7()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_uuid_v7() {
        let id = new_id();
        assert_eq!(id.get_version_num(), 7);
    }

    #[test]
    fn generated_ids_are_unique() {
        let mut ids: Vec<Uuid> = (0..1000).map(|_| new_id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn uuid_v7_is_time_ordered() {
        let a = new_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = new_id();
        assert!(a < b, "v7 ids should be time ordered: {a} < {b}");
    }
}
