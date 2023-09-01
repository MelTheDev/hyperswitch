use api_models::admin::MerchantConnectorWebhookDetails;
pub use api_models::webhooks::{
    IncomingWebhookDetails, IncomingWebhookEvent, MerchantWebhookConfig, ObjectReferenceId,
    OutgoingWebhook, OutgoingWebhookContent, WebhookFlow,
};
use common_utils::ext_traits::ValueExt;
use error_stack::ResultExt;
use masking::ExposeInterface;

use super::ConnectorCommon;
use crate::{
    core::errors::{self, CustomResult},
    db::StorageInterface,
    logger, services,
    types::domain,
    utils::{self, crypto},
};

pub struct IncomingWebhookRequestDetails<'a> {
    pub method: actix_web::http::Method,
    pub headers: &'a actix_web::http::header::HeaderMap,
    pub body: &'a [u8],
    pub query_params: String,
}

#[async_trait::async_trait]
pub trait IncomingWebhook: ConnectorCommon + Sync {
    fn get_webhook_body_decoding_algorithm(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<Box<dyn crypto::DecodeMessage + Send>, errors::ConnectorError> {
        Ok(Box::new(crypto::NoAlgorithm))
    }

    async fn get_webhook_body_decoding_merchant_secret(
        &self,
        _db: &dyn StorageInterface,
        _merchant_id: &str,
    ) -> CustomResult<Vec<u8>, errors::ConnectorError> {
        Ok(Vec::new())
    }

    fn get_webhook_body_decoding_message(
        &self,
        request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<Vec<u8>, errors::ConnectorError> {
        Ok(request.body.to_vec())
    }

    async fn decode_webhook_body(
        &self,
        db: &dyn StorageInterface,
        request: &IncomingWebhookRequestDetails<'_>,
        merchant_id: &str,
    ) -> CustomResult<Vec<u8>, errors::ConnectorError> {
        let algorithm = self.get_webhook_body_decoding_algorithm(request)?;

        let message = self
            .get_webhook_body_decoding_message(request)
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;
        let secret = self
            .get_webhook_body_decoding_merchant_secret(db, merchant_id)
            .await
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

        algorithm
            .decode_message(&secret, message.into())
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)
    }

    fn get_webhook_source_verification_algorithm(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<Box<dyn crypto::VerifySignature + Send>, errors::ConnectorError> {
        Ok(Box::new(crypto::NoAlgorithm))
    }

    async fn get_webhook_source_verification_merchant_secret(
        &self,
        db: &dyn StorageInterface,
        merchant_account: &domain::MerchantAccount,
        connector_name: &str,
        key_store: &domain::MerchantKeyStore,
        object_reference_id: ObjectReferenceId,
    ) -> CustomResult<api_models::webhooks::ConnectorWebhookSecrets, errors::ConnectorError> {
        let merchant_id = merchant_account.merchant_id.as_str();
        let debug_suffix = format!(
            "For merchant_id: {}, and connector_name: {}",
            merchant_id, connector_name
        );
        let default_secret = "default_secret".to_string();
        let connector_label = utils::get_connector_label_using_object_reference_id(
            db,
            object_reference_id,
            merchant_account,
            connector_name,
        )
        .await
        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
        .attach_printable("Error while fetching connector_label")?;
        let merchant_connector_account_result = db
            .find_merchant_connector_account_by_merchant_id_connector_label(
                merchant_id,
                &connector_label,
                key_store,
            )
            .await;

        let connector_webhook_secrets = match merchant_connector_account_result {
            Ok(mca) => match mca.connector_webhook_details {
                Some(merchant_connector_webhook_details) => {
                    let connector_webhook_details = merchant_connector_webhook_details
                        .parse_value::<MerchantConnectorWebhookDetails>(
                            "MerchantConnectorWebhookDetails",
                        )
                        .change_context_lazy(|| {
                            errors::ConnectorError::WebhookSourceVerificationFailed
                        })
                        .attach_printable_lazy(|| {
                            format!(
                                "Deserializing MerchantConnectorWebhookDetails failed {}",
                                debug_suffix
                            )
                        })?;
                    api_models::webhooks::ConnectorWebhookSecrets {
                        secret: connector_webhook_details
                            .merchant_secret
                            .expose()
                            .into_bytes(),
                        additional_secret: connector_webhook_details
                            .additional_secret
                            .map(|secret| secret.expose()),
                    }
                }
                None => api_models::webhooks::ConnectorWebhookSecrets {
                    secret: default_secret.into_bytes(),
                    additional_secret: None,
                },
            },
            Err(err) => {
                logger::error!(
                    "Failed to fetch merchant_secret for source verification {}",
                    debug_suffix
                );
                logger::error!("DB error = {:?}", err);
                api_models::webhooks::ConnectorWebhookSecrets {
                    secret: default_secret.into_bytes(),
                    additional_secret: None,
                }
            }
        };

        //need to fetch merchant secret from config table with caching in future for enhanced performance

        //If merchant has not set the secret for webhook source verification, "default_secret" is returned.
        //So it will fail during verification step and goes to psync flow.
        Ok(connector_webhook_secrets)
    }

    fn get_webhook_source_verification_signature(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
        _connector_webhook_secrets: &api_models::webhooks::ConnectorWebhookSecrets,
    ) -> CustomResult<Vec<u8>, errors::ConnectorError> {
        Ok(Vec::new())
    }

    fn get_webhook_source_verification_message(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
        _merchant_id: &str,
        _connector_webhook_secrets: &api_models::webhooks::ConnectorWebhookSecrets,
    ) -> CustomResult<Vec<u8>, errors::ConnectorError> {
        Ok(Vec::new())
    }

    async fn verify_webhook_source(
        &self,
        db: &dyn StorageInterface,
        request: &IncomingWebhookRequestDetails<'_>,
        merchant_account: &domain::MerchantAccount,
        connector_label: &str,
        key_store: &domain::MerchantKeyStore,
        object_reference_id: ObjectReferenceId,
    ) -> CustomResult<bool, errors::ConnectorError> {
        let algorithm = self
            .get_webhook_source_verification_algorithm(request)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        let connector_webhook_secrets = self
            .get_webhook_source_verification_merchant_secret(
                db,
                merchant_account,
                connector_label,
                key_store,
                object_reference_id,
            )
            .await
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        let signature = self
            .get_webhook_source_verification_signature(request, &connector_webhook_secrets)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        let message = self
            .get_webhook_source_verification_message(
                request,
                &merchant_account.merchant_id,
                &connector_webhook_secrets,
            )
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_object_reference_id(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<ObjectReferenceId, errors::ConnectorError>;

    fn get_webhook_event_type(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<IncomingWebhookEvent, errors::ConnectorError>;

    fn get_webhook_resource_object(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<serde_json::Value, errors::ConnectorError>;

    fn get_webhook_api_response(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<services::api::ApplicationResponse<serde_json::Value>, errors::ConnectorError>
    {
        Ok(services::api::ApplicationResponse::StatusOk)
    }

    fn get_dispute_details(
        &self,
        _request: &IncomingWebhookRequestDetails<'_>,
    ) -> CustomResult<super::disputes::DisputePayload, errors::ConnectorError> {
        Err(errors::ConnectorError::NotImplemented("get_dispute_details method".to_string()).into())
    }
}
