pub use serde_json::json as impl_json_inner;

#[macro_export]
macro_rules! json {
    ($ty:ty { $($items:tt)* }) => {
        {
            <$ty as serde::Deserialize>::deserialize($crate::helpers::impl_json_inner!({$($items)*})).unwrap()
        }
    };
}
