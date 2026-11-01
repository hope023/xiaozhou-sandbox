use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error("密码错误")]
    BadPassword,
    #[error("保险库未解锁")]
    Locked,
    #[error("数据已损坏或被篡改")]
    Corrupt,
    #[error("找不到该项目")]
    NotFound,
    #[error("读写失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("格式错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("加密运算失败")]
    Crypto,
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
