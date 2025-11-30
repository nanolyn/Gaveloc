use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::{extract::Path, http::StatusCode, Router};
use gaveloc_core::ports::OtpListener;
use gaveloc_core::Error;
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, info, warn};

const OTP_PORT: u16 = 4646;

/// Local HTTP server for receiving OTP from mobile app
pub struct HttpOtpListener {
    running: Arc<AtomicBool>,
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl HttpOtpListener {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for HttpOtpListener {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OtpListener for HttpOtpListener {
    async fn start(&self) -> Result<oneshot::Receiver<String>, Error> {
        if self.running.load(Ordering::SeqCst) {
            return Err(Error::Other("OTP listener already running".to_string()));
        }

        let (otp_tx, otp_rx) = oneshot::channel::<String>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let otp_tx = Arc::new(Mutex::new(Some(otp_tx)));
        let running = self.running.clone();

        // Store shutdown sender
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // Build router
        // Endpoint: /ffxivlauncher/{otp}
        let otp_tx_clone = otp_tx.clone();
        let running_clone = running.clone();

        let app = Router::new().route(
            "/ffxivlauncher/{otp}",
            axum::routing::get(move |Path(otp): Path<String>| {
                let otp_tx = otp_tx_clone.clone();
                let running = running_clone.clone();
                async move {
                    info!(otp_length = otp.len(), "received OTP from mobile app");

                    // Send OTP through channel
                    if let Some(tx) = otp_tx.lock().await.take() {
                        let _ = tx.send(otp);
                    }

                    running.store(false, Ordering::SeqCst);

                    (StatusCode::OK, "OTP received. You can close this window.")
                }
            }),
        );

        // Start server
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], OTP_PORT));

        running.store(true, Ordering::SeqCst);

        info!(port = OTP_PORT, "starting OTP listener");

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    warn!(error = %e, "failed to bind OTP listener");
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                    debug!("OTP listener shutting down");
                })
                .await
                .ok();

            running.store(false, Ordering::SeqCst);
        });

        Ok(otp_rx)
    }

    async fn stop(&self) -> Result<(), Error> {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(());
        }
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_otp_listener_lifecycle() {
        let listener = HttpOtpListener::new();

        assert!(!listener.is_running());

        // Start listener
        let _rx = listener.start().await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(listener.is_running());

        // Stop listener
        listener.stop().await.unwrap();

        // Give server time to stop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(!listener.is_running());
    }

    #[tokio::test]
    async fn test_cannot_start_twice() {
        let listener = HttpOtpListener::new();

        let _rx1 = listener.start().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Second start should fail
        let result = listener.start().await;
        assert!(result.is_err());

        listener.stop().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_otp_reception() {
        let listener = HttpOtpListener::new();
        let otp_rx = listener.start().await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Send OTP via HTTP
        let client = reqwest::Client::new();
        let response = client
            .get("http://127.0.0.1:4646/ffxivlauncher/123456")
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());

        // Verify OTP received
        let otp = otp_rx.await.unwrap();
        assert_eq!(otp, "123456");

        // Listener should have stopped
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(!listener.is_running());
    }
}
