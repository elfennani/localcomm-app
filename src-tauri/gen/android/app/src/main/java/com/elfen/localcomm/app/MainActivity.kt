package com.elfen.localcomm.app

import android.net.nsd.NsdManager
import android.os.Bundle
import android.os.Environment
import android.util.Log
import android.widget.Toast
import androidx.activity.enableEdgeToEdge
import androidx.core.content.getSystemService
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val TAG = "MainActivity"

class MainActivity : TauriActivity() {
  external fun startService(download_path: String, app_data_path: String)
  external fun stopService()

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    val downloadDir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)

    lifecycleScope.launch(Dispatchers.IO) {
      startService(
        downloadDir.absolutePath,
        filesDir.absolutePath
      )
      
      Log.d(TAG, "Service Stopped!")
    }
  }

  override fun onDestroy() {
    super.onDestroy()

    stopService()
  }
}
