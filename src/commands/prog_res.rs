use crate::commands::display_table::ResultingData;

pub enum ProgramRes {
    Data(Box<dyn ResultingData>),
    Str(String),
    Idle,
}

impl From<String> for ProgramRes {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl<T> From<T> for ProgramRes
where
    T: ResultingData + 'static,
{
    fn from(s: T) -> Self {
        Self::Data(Box::new(s))
    }
}

impl From<()> for ProgramRes {
    fn from(_: ()) -> Self {
        Self::Idle
    }
}
