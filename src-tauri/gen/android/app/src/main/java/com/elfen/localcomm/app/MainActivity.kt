package com.elfen.localcomm.app

import android.net.nsd.NsdManager
import android.os.Bundle
import android.widget.Toast
import androidx.activity.enableEdgeToEdge
import androidx.core.content.getSystemService
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    
    lifecycleScope.launch(Dispatchers.IO) {
      hello(filesDir.absolutePath);
    }
  }

  external fun hello(absolutePath: String): String
}
