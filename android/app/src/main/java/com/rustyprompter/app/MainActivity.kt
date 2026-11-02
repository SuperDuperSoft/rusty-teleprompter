package com.rustyprompter.app

import android.Manifest
import android.app.Activity
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.AtomicFile
import android.view.Gravity
import android.view.KeyEvent
import android.view.WindowInsets
import android.view.WindowManager
import android.webkit.*
import android.widget.FrameLayout
import android.widget.TextView
import org.java_websocket.client.WebSocketClient
import org.java_websocket.handshake.ServerHandshake
import org.json.JSONObject
import org.vosk.Model
import org.vosk.Recognizer
import org.vosk.android.RecognitionListener
import org.vosk.android.SpeechService
import java.io.ByteArrayInputStream
import java.io.File
import java.net.HttpURLConnection
import java.net.URI
import java.util.concurrent.Executors

/** Platform adapter only. The UI, matching, timing and revision logic are Rust/WASM. */
class MainActivity : Activity() {
    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private val audioWorker = Executors.newSingleThreadExecutor()
    private val prefs by lazy { getSharedPreferences("rustyprompter", MODE_PRIVATE) }
    private val cache by lazy { AtomicFile(File(filesDir, "library.json")) }
    private lateinit var web: WebView
    private lateinit var loading: TextView
    private var model: Model? = null
    private var recognizer: Recognizer? = null
    private var service: SpeechService? = null
    private var foreground = false
    private var destroyed = false
    private var wantedAudio = false
    private var permissionPrompt = false
    private var prompting = false
    private var volumeKeys = false
    private var grammar: String? = null
    private var audioGeneration = 0
    private var building = -1
    private var take: Long? = null
    private var retries = 0
    private var socket: WebSocketClient? = null
    @Volatile private var epoch = 0
    private var retry = 1000L
    @Volatile private var voiceReady = false
    @Volatile private var voiceStatus = "Preparing offline voice model…"
    @Volatile private var connection = "Not paired · scripts work offline"

    @android.annotation.SuppressLint("SetJavaScriptEnabled", "JavascriptInterface")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setDecorFitsSystemWindows(false) // same edge-to-edge layout on Android 11-16
        applyOrientation(prefs.getString("settings","{}").orEmpty())
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val root = FrameLayout(this)
        web = WebView(this).apply {
            setBackgroundColor(Color.rgb(16,24,23))
            settings.javaScriptEnabled = true; settings.domStorageEnabled = true; settings.textZoom = 100
            settings.allowFileAccess = false; settings.allowContentAccess = false
            settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            addJavascriptInterface(Bridge(), "RustyNative")
            webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?) = request?.url?.host != "appassets.androidplatform.net"
                override fun shouldInterceptRequest(view: WebView?, request: WebResourceRequest?): WebResourceResponse {
                    val path = request?.url?.path?.trimStart('/') ?: "index.html"
                    if (request?.url?.host != "appassets.androidplatform.net" || path.split('/').any { it == ".." }) return WebResourceResponse("text/plain", "utf-8", 403, "Forbidden", emptyMap(), ByteArrayInputStream(byteArrayOf()))
                    val mime = when (path.substringAfterLast('.', "")) { "html" -> "text/html"; "js" -> "application/javascript"; "wasm" -> "application/wasm"; "css" -> "text/css"; "svg" -> "image/svg+xml"; "png" -> "image/png"; else -> "application/octet-stream" }
                    return try { WebResourceResponse(mime, if (mime.startsWith("text/") || mime == "application/javascript") "utf-8" else null, assets.open("web/$path")) }
                    catch (_: Exception) { WebResourceResponse("text/plain", "utf-8", 404, "Not found", emptyMap(), ByteArrayInputStream(byteArrayOf())) }
                }
            }
        }
        root.setBackgroundColor(Color.rgb(16,24,23))
        // Android 15+ draws apps edge-to-edge. Pad the content so nothing sits
        // under the status bar, camera cutout, gesture bar or keyboard.
        root.setOnApplyWindowInsetsListener { view, insets ->
            val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            val ime = insets.getInsets(WindowInsets.Type.ime())
            view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
            WindowInsets.CONSUMED
        }
        root.addView(web, FrameLayout.LayoutParams(-1,-1))
        loading = TextView(this).apply { text="RustyPrompter\n\nLoading…"; textSize=22f; setTextColor(Color.rgb(232,186,140)); gravity=Gravity.CENTER; setBackgroundColor(Color.rgb(16,24,23)) }
        root.addView(loading, FrameLayout.LayoutParams(-1,-1)); setContentView(root)
        web.loadUrl("https://appassets.androidplatform.net/index.html")
        loadModel(); connect()
    }
    inner class Bridge {
        @JavascriptInterface fun initialState(): String {
            val library = try { JSONObject(cache.openRead().bufferedReader().use { it.readText() }) } catch (_: Exception) { JSONObject.NULL }
            return JSONObject().put("native",true).put("library",library).put("settings",jsonPref("settings")).put("position",jsonPref("position")).put("voiceReady",voiceReady).put("voiceStatus",voiceStatus).put("connection",connection).put("endpoint",prefs.getString("endpoint","")).toString()
        }
        @JavascriptInterface fun command(name:String,payload:String) {
            main.post {
                if (destroyed) return@post
                when(name) {
                    "ui_ready" -> loading.visibility=android.view.View.GONE
                    "start" -> { payload.trim().toLongOrNull()?.let{take=it}; retries=0; wantedAudio=true; startAudio() }
                    "reset" -> { payload.trim().toLongOrNull()?.let{take=it}; service?.reset() }
                    "stop" -> stopAudio()
                    "settings" -> { prefs.edit().putString("settings",payload).apply(); applyOrientation(payload) }
                    "position" -> prefs.edit().putString("position",payload).apply()
                    "grammar" -> { grammar=payload.ifBlank{null}; if(service!=null||building==audioGeneration){stopAudio(false);startAudio()} }
                    "remote" -> try { val value=JSONObject(payload); prompting=value.optBoolean("prompting"); volumeKeys=value.optBoolean("volume"); if(prompting)web.requestFocus() } catch(_:Exception){}
                    "pair" -> try { val value=JSONObject(payload); pair(value.optString("address"),value.optString("code")) } catch (_:Exception){emit(JSONObject().put("type","pair_error").put("message","Check the PC address and code."))}
                    "status" -> try { val value=JSONObject(payload).put("type","status"); socket?.takeIf{it.isOpen}?.send(value.toString()) }catch(_:Exception){}
                }
            }
        }
    }
    /** Auto follows the phone's rotation (and its rotation lock). Portrait/landscape stay locked. */
    private fun applyOrientation(settings:String){
        val value=try{JSONObject(settings)}catch(_:Exception){JSONObject()}
        val wanted=when(value.optString("orientation")){"portrait"->ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT;"landscape"->ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE;else->ActivityInfo.SCREEN_ORIENTATION_FULL_USER}
        if(requestedOrientation!=wanted)requestedOrientation=wanted
    }
    /** Raw key forwarding while prompting; Rust's Remote decides what a key means. */
    override fun dispatchKeyEvent(event:KeyEvent):Boolean{
        val key=when(event.keyCode){
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,KeyEvent.KEYCODE_HEADSETHOOK->"MediaPlayPause";KeyEvent.KEYCODE_MEDIA_PLAY->"MediaPlay";KeyEvent.KEYCODE_MEDIA_PAUSE->"MediaPause"
            KeyEvent.KEYCODE_MEDIA_NEXT->"MediaTrackNext";KeyEvent.KEYCODE_MEDIA_PREVIOUS->"MediaTrackPrevious";KeyEvent.KEYCODE_PAGE_UP->"PageUp";KeyEvent.KEYCODE_PAGE_DOWN->"PageDown"
            KeyEvent.KEYCODE_DPAD_UP->"ArrowUp";KeyEvent.KEYCODE_DPAD_DOWN->"ArrowDown";KeyEvent.KEYCODE_DPAD_LEFT->"ArrowLeft";KeyEvent.KEYCODE_DPAD_RIGHT->"ArrowRight"
            KeyEvent.KEYCODE_SPACE->" ";KeyEvent.KEYCODE_ENTER,KeyEvent.KEYCODE_DPAD_CENTER,KeyEvent.KEYCODE_NUMPAD_ENTER->"Enter";KeyEvent.KEYCODE_B->"b";KeyEvent.KEYCODE_PERIOD->"."
            KeyEvent.KEYCODE_VOLUME_UP->if(volumeKeys)"AudioVolumeUp"else null;KeyEvent.KEYCODE_VOLUME_DOWN->if(volumeKeys)"AudioVolumeDown"else null
            else->null
        }
        if(!prompting||key==null||event.isCtrlPressed||event.isAltPressed||event.isMetaPressed||(event.action!=KeyEvent.ACTION_DOWN&&event.action!=KeyEvent.ACTION_UP))return super.dispatchKeyEvent(event)
        emit(JSONObject().put("type","key").put("key",key).put("down",event.action==KeyEvent.ACTION_DOWN).put("repeat",event.repeatCount>0));return true
    }
    private fun jsonPref(name:String):Any = try { JSONObject(prefs.getString(name,"{}") ?: "{}") } catch(_:Exception){JSONObject()}
    private fun emit(event:JSONObject) { main.post { if(!destroyed&&::web.isInitialized) web.evaluateJavascript("window.__rustyEvents=window.__rustyEvents||[];window.__rustyEvents.push(JSON.parse(${JSONObject.quote(event.toString())}));",null) } }
    private fun connection(message:String){connection=message;emit(JSONObject().put("type","connection").put("message",message))}
    @Synchronized private fun saveLibrary(value:JSONObject){val clean=JSONObject().put("generation",value.optLong("generation")).put("scripts",value.getJSONArray("scripts"));val stream=cache.startWrite();try{stream.write(clean.toString().toByteArray());cache.finishWrite(stream)}catch(e:Exception){cache.failWrite(stream);throw e};emit(clean.put("type","library"))}
    private fun pair(raw:String,code:String){
        epoch++;socket?.close();socket=null;connection("Connecting to PC…")
        worker.execute{
            try{
                val address=if(raw.contains("://"))raw.trim().trimEnd('/')else"http://${raw.trim().trimEnd('/')}"
                val uri=URI(address);require(uri.scheme in listOf("http","https")&&!uri.host.isNullOrBlank()&&uri.userInfo==null&&uri.query==null&&uri.fragment==null&&uri.path.isNullOrEmpty()){"Use the PC address shown in RustyPrompter."}
                val endpoint=URI(uri.scheme,null,uri.host,if(uri.port<0)8766 else uri.port,null,null,null).toString()
                val request=URI("$endpoint/api/pair").toURL().openConnection()as HttpURLConnection
                val value:JSONObject
                try{request.requestMethod="POST";request.connectTimeout=7000;request.readTimeout=7000;request.doOutput=true;request.setRequestProperty("Content-Type","application/json");request.outputStream.use{it.write(JSONObject().put("code",code).toString().toByteArray())};val ok=request.responseCode in 200..299;val text=(if(ok)request.inputStream else request.errorStream).bufferedReader().use{it.readText()};value=JSONObject(text);require(ok){value.optString("detail","Could not pair with PC")}}finally{request.disconnect()}
                saveLibrary(value);prefs.edit().putString("endpoint",endpoint).putString("token",value.getString("token")).apply()
                main.post{if(!destroyed){retry=1000;connect();emit(JSONObject().put("type","paired"))}}
            }catch(e:Exception){main.post{if(!destroyed){connect();emit(JSONObject().put("type","pair_error").put("message","${e.message}\nCheck Wi-Fi and Windows Firewall."))}}}
        }
    }
    private fun connect(){
        if(destroyed)return
        val endpoint=prefs.getString("endpoint","").orEmpty();val token=prefs.getString("token","").orEmpty();epoch++;val generation=epoch;socket?.close();socket=null
        if(endpoint.isEmpty()||token.isEmpty()){connection("Not paired · scripts work offline");return}
        connection("Connecting to PC…")
        val client=object:WebSocketClient(URI(endpoint.replaceFirst("http","ws")+"/ws"),mapOf("Authorization" to "Bearer $token")){
            override fun onOpen(handshake:ServerHandshake?){main.post{if(generation==epoch){retry=1000;connection("Connected to PC")}}}
            override fun onMessage(message:String?){if(generation!=epoch||message==null)return;try{val value=JSONObject(message);if(value.optString("type")=="library")saveLibrary(value)}catch(_:Exception){connection("Sync failed · cached scripts retained")}}
            override fun onClose(code:Int,reason:String?,remote:Boolean){main.post{if(generation!=epoch||destroyed)return@post;if(code==4401||reason.orEmpty().contains("401")||reason.orEmpty().contains("403")){connection("Pairing expired · connect to PC again")}else{connection("PC offline · reading works offline");val delay=retry;retry=(retry*2).coerceAtMost(30000);main.postDelayed({if(generation==epoch&&!destroyed)connect()},delay)}}}
            override fun onError(error:Exception?){}
        }
        client.setConnectionLostTimeout(15);socket=client
        try{client.connect()}catch(_:Exception){connection("Could not connect to PC");main.postDelayed({if(generation==epoch&&!destroyed)connect()},3000)}
    }
    private fun loadModel(){audioWorker.execute{try{
        val folder=modelFolder();val marker=File(folder,".complete");if(!marker.exists()){copyAssets("model-en-us",folder);marker.writeText("1")}
        val loaded=Model(folder.absolutePath);main.post{if(destroyed)loaded.close()else{model=loaded;voiceReady=true;voiceStatus="Offline voice model ready";emit(JSONObject().put("type","voice").put("ready",true).put("message",voiceStatus))}}
    }catch(error:Throwable){main.post{voiceStatus="Offline voice model could not load. Close other apps and reopen RustyPrompter.";emit(JSONObject().put("type","voice").put("ready",false).put("message",voiceStatus))}}}}
    private fun copyAssets(path:String,target:File){val names=assets.list(path).orEmpty();if(names.isEmpty())assets.open(path).use{source->target.outputStream().use{source.copyTo(it)}}else{target.mkdirs();for(name in names)copyAssets("$path/$name",File(target,name))}}
    private fun modelFolder()=File(filesDir,"vosk-model-en-us-0.15")
    private fun startAudio(){
        if(!foreground||!voiceReady||!wantedAudio||service!=null)return
        if(checkSelfPermission(Manifest.permission.RECORD_AUDIO)!=PackageManager.PERMISSION_GRANTED){permissionPrompt=true;requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO),101);return}
        stopAudio(false);val generation=++audioGeneration;building=generation;val loaded=model;val words=grammar
        // A 3000-word grammar compiles slowly, so the recognizer is built off the main thread.
        audioWorker.execute{
            val built=try{buildRecognizer(loaded,words)}catch(e:Exception){main.post{audioFailed(generation,e.message?:"Could not open microphone")};return@execute}
            main.post{
                if(generation!=audioGeneration||!wantedAudio||destroyed){built.close();return@post}
                recognizer=built;listen(generation,built)
            }
        }
    }
    /** Script vocabulary when the model has runtime graphs; otherwise the full vocabulary and a grammar_error. Runs on audioWorker. */
    private fun buildRecognizer(loaded:Model?,words:String?):Recognizer{
        if(words!=null){
            val graph=File(modelFolder(),"graph")
            if(File(graph,"Gr.fst").exists()&&File(graph,"HCLr.fst").exists())try{return Recognizer(loaded,16000f,words)}catch(_:Exception){}
            main.post{if(grammar==words)grammar=null}
            emit(JSONObject().put("type","grammar_error").put("message","Script vocabulary is not supported by this voice model. Using full vocabulary."))
        }
        return Recognizer(loaded,16000f)
    }
    private fun audioFailed(generation:Int,message:String){if(generation!=audioGeneration)return;if(retryAudio())return;stopAudio();emit(JSONObject().put("type","audio_error").put("message",message))}
    private fun listen(generation:Int,built:Recognizer){
        try{service=SpeechService(built,16000f);service!!.startListening(object:RecognitionListener{
            private fun result(value:String?,final:Boolean){if(generation!=audioGeneration||!wantedAudio)return;try{val json=JSONObject(value?:"{}");val text=json.optString(if(final)"text"else"partial");if(text.isNotBlank()){retries=0;val event=JSONObject().put("type","speech").put("text",text).put("final",final);take?.let{event.put("take",it)};emit(event)}}catch(_:Exception){}}
            override fun onPartialResult(hypothesis:String?)=result(hypothesis,false)
            override fun onResult(hypothesis:String?)=result(hypothesis,true)
            override fun onFinalResult(hypothesis:String?)=result(hypothesis,true)
            override fun onError(exception:Exception?)=audioFailed(generation,exception?.message?:"Microphone unavailable")
            override fun onTimeout()=audioFailed(generation,"Listening stopped. Tap Resume to restart.")
        })}catch(e:Exception){audioFailed(generation,e.message?:"Could not open microphone")}
    }
    /** Restart a failed recognizer with backoff (500/1000/2000 ms). False once the retries are used up. */
    private fun retryAudio():Boolean{
        if(!wantedAudio||retries>=3)return false
        stopAudio(false);main.postDelayed({if(wantedAudio&&!destroyed)startAudio()},longArrayOf(500,1000,2000)[retries++]);return true
    }
    private fun stopAudio(clearWanted:Boolean=true){audioGeneration++;if(clearWanted)wantedAudio=false;service?.stop();service?.shutdown();service=null;recognizer?.close();recognizer=null}
    override fun onResume(){super.onResume();foreground=true;if(wantedAudio)startAudio()}
    override fun onPause(){foreground=false;stopAudio(!permissionPrompt);emit(JSONObject().put("type","pause"));super.onPause()}
    override fun onRequestPermissionsResult(code:Int,permissions:Array<out String>,results:IntArray){super.onRequestPermissionsResult(code,permissions,results);if(code==101){permissionPrompt=false;if(results.firstOrNull()==PackageManager.PERMISSION_GRANTED){wantedAudio=true;emit(JSONObject().put("type","permission_granted"));startAudio()}else{stopAudio();emit(JSONObject().put("type","audio_error").put("message","Allow microphone access in Android settings to follow your voice. Manual scrolling and Next still work."))}}}
    override fun onDestroy(){destroyed=true;epoch++;socket?.close();stopAudio();val loaded=model;model=null;worker.shutdown();audioWorker.execute{loaded?.close()};audioWorker.shutdown();if(::web.isInitialized){web.removeJavascriptInterface("RustyNative");web.destroy()};super.onDestroy()}
}
