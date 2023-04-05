use async_trait::async_trait;
use common_utils::{
    crypto,
    errors::{self, CustomResult},
};
use error_stack::{IntoReport, ResultExt};
use masking::{PeekInterface, Secret};
use storage_models::encryption::Encryption;

#[async_trait]
pub trait TypeEncryption<
    T,
    V: crypto::EncodeMessage + crypto::DecodeMessage,
    S: masking::Strategy<T>,
>: Sized
{
    async fn encrypt(
        masked_data: Secret<T, S>,
        key: &[u8],
        crypt_algo: V,
    ) -> CustomResult<Self, errors::CryptoError>;
    async fn decrypt(
        encrypted_data: Encryption,
        key: &[u8],
        crypt_algo: V,
    ) -> CustomResult<Self, errors::CryptoError>;
}

#[async_trait]
impl<
        V: crypto::DecodeMessage + crypto::EncodeMessage + Send + 'static,
        S: masking::Strategy<String> + Send,
    > TypeEncryption<String, V, S> for crypto::Encryptable<Secret<String, S>>
{
    async fn encrypt(
        masked_data: Secret<String, S>,
        key: &[u8],
        crypt_algo: V,
    ) -> CustomResult<Self, errors::CryptoError> {
        let encrypted_data = crypt_algo.encode_message(key, masked_data.peek().as_bytes())?;

        Ok(Self::new(masked_data, encrypted_data))
    }

    async fn decrypt(
        encrypted_data: Encryption,
        key: &[u8],
        crypt_algo: V,
    ) -> CustomResult<Self, errors::CryptoError> {
        let encrypted = encrypted_data.into_inner();
        let decrypted_data = crypt_algo.decode_message(key, encrypted.clone())?;
        let value: String = std::str::from_utf8(&decrypted_data)
            .into_report()
            .change_context(errors::CryptoError::DecodingFailed)?
            .to_string();

        Ok(Self::new(value.into(), encrypted))
    }
}

pub async fn get_key_and_algo(
    _db: &dyn crate::db::StorageInterface,
    _merchant_id: String,
) -> CustomResult<Vec<u8>, crate::core::errors::StorageError> {
    Ok(Vec::new())
}