#[cfg(target_os = "android")]
use std::sync::{
   Arc,
   atomic::{AtomicBool, Ordering},
};
#[cfg(target_os = "android")]
use tauri::{Manager, Runtime, Webview};

#[derive(Clone)]
pub(crate) struct AndroidJpeg {
   #[cfg(target_os = "android")]
   started: Arc<AtomicBool>,
   result: tokio::sync::watch::Sender<Option<std::result::Result<(), String>>>,
}

impl Default for AndroidJpeg {
   fn default() -> Self {
      Self {
         #[cfg(target_os = "android")]
         started: Arc::new(AtomicBool::new(false)),
         result: tokio::sync::watch::channel(None).0,
      }
   }
}

impl AndroidJpeg {
   pub(crate) async fn wait(&self) -> crate::Result<()> {
      let mut receiver = self.result.subscribe();
      let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
         loop {
            if let Some(result) = receiver.borrow().clone() {
               return result;
            }
            receiver
               .changed()
               .await
               .map_err(|_| "Android JPEG bootstrap channel closed".to_string())?;
         }
      })
      .await
      .map_err(|_| crate::Error::Custom("Android JPEG bootstrap timed out".into()))?;
      result.map_err(crate::Error::Custom)
   }
}

#[cfg(target_os = "android")]
pub(crate) fn on_webview_ready<R: Runtime>(webview: Webview<R>) {
   let state = webview
      .state::<crate::commands::ThumbnailSessions>()
      .android_jpeg
      .clone();
   if state.started.swap(true, Ordering::AcqRel) {
      return;
   }
   let dispatched = state.clone();
   let result = webview.with_webview(move |platform| {
      let completion = dispatched.result.clone();
      platform.jni_handle().exec(move |env, activity, _| {
         let result = (|| -> std::result::Result<(), String> {
            let name = env
               .new_string("com.plugin.mediaparser.BoundedJpegOutputStream")
               .map_err(|error| error.to_string())?;
            let class = env
               .call_method(
                  activity,
                  "getAppClass",
                  "(Ljava/lang/String;)Ljava/lang/Class;",
                  &[jni::objects::JValue::Object(&name)],
               )
               .and_then(|value| value.l())
               .and_then(|class| env.new_global_ref(class))
               .map_err(|error| error.to_string())?;
            let vm = env.get_java_vm().map_err(|error| error.to_string())?;
            media_parser::initialize_android_jpeg(vm, class)
         })();
         if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
         }
         completion.send_replace(Some(result));
      });
   });
   if let Err(error) = result {
      state
         .result
         .send_replace(Some(Err(format!("Android JPEG WebView dispatch: {error}"))));
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[tokio::test]
   async fn wait_propagates_stored_error() {
      let state = AndroidJpeg::default();
      state
         .result
         .send_replace(Some(Err("bootstrap failed".to_owned())));

      let error = state.wait().await.expect_err("stored error should fail");

      assert!(matches!(
         error,
         crate::Error::Custom(message) if message == "bootstrap failed"
      ));
   }

   #[tokio::test(start_paused = true)]
   async fn wait_times_out() {
      let error = AndroidJpeg::default()
         .wait()
         .await
         .expect_err("missing bootstrap result should time out");

      assert!(matches!(
         error,
         crate::Error::Custom(message) if message == "Android JPEG bootstrap timed out"
      ));
   }
}
