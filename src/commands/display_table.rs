use crate::util::output::print_table;
use serde::Serialize;
use serde_json::Value;

pub trait DisplayTable {
    fn headers(&self) -> Vec<&'static str>;

    fn rows(&self) -> Vec<Vec<String>>;

    fn display_as_table(&self) {
        print_table(&self.headers(), self.rows());
    }
}

pub trait ResultingData: DisplayTable {
    fn as_json(&self) -> Result<Value, serde_json::error::Error>;
}

impl<T: DisplayTable + Serialize> ResultingData for T {
    fn as_json(&self) -> Result<Value, serde_json::error::Error> {
        serde_json::to_value(self)
    }
}
