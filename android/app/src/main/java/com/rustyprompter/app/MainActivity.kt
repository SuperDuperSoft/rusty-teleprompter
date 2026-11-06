package com.rustyprompter.app

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.ContentValues
import android.graphics.Matrix
import android.graphics.RectF
import android.graphics.SurfaceTexture
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.params.OutputConfiguration
import android.hardware.camera2.params.SessionConfiguration
import android.media.MediaRecorder
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.HandlerThread
import android.os.ParcelFileDescriptor
import android.provider.MediaStore
import android.util.Range
import android.util.Size
import android.view.Surface
import android.view.TextureView
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
    private lateinit var root: FrameLayout
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
        root = FrameLayout(this)
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
                    "settings" -> { prefs.edit().putString("settings",payload).apply(); if(!orientationLocked)applyOrientation(payload) }
                    "position" -> prefs.edit().putString("position",payload).apply()
                    "grammar" -> { grammar=payload.ifBlank{null}; if(service!=null||building==audioGeneration){stopAudio(false);startAudio()} }
                    "remote" -> try { val value=JSONObject(payload); prompting=value.optBoolean("prompting"); volumeKeys=value.optBoolean("volume"); if(prompting)web.requestFocus() } catch(_:Exception){}
                    "pair" -> try { val value=JSONObject(payload); pair(value.optString("address"),value.optString("code")) } catch (_:Exception){emit(JSONObject().put("type","pair_error").put("message","Check the PC address and code."))}
                    "save_library" -> try { saveLibrary(JSONObject(payload),false) } catch(_:Exception){}
                    "push" -> try { push(JSONObject(payload)) } catch(_:Exception){}
                    "delete" -> try { val value=JSONObject(payload); deleteScript(value.getString("id"),value.getLong("revision")) } catch(_:Exception){}
                    "refresh" -> refresh()
                    "camera_open" -> cameraOpen(try { JSONObject(payload).optBoolean("front", true) } catch (_: Exception) { true })
                    "camera_close" -> cameraClose()
                    "camera_record" -> { val name = try { JSONObject(payload).optString("name") } catch (_: Exception) { "" }; onCamera { startRecording(name.ifBlank { "RustyPrompter" }) } }
                    "camera_stop" -> onCamera { stopRecording(); startPreview() }
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
    /** Writes the offline cache; `notify` sends it to the UI (PC libraries do, tablet edits already are there). */
    @Synchronized private fun saveLibrary(value:JSONObject,notify:Boolean=true){val clean=JSONObject().put("generation",value.optLong("generation")).put("scripts",value.getJSONArray("scripts"));val stream=cache.startWrite();try{stream.write(clean.toString().toByteArray());cache.finishWrite(stream)}catch(e:Exception){cache.failWrite(stream);throw e};if(notify)emit(clean.put("type","library"))}
    /** One authenticated call to the paired PC: (HTTP status, body). Runs on `worker`. */
    private fun device(method:String,path:String,body:String?):Pair<Int,String>{
        val endpoint=prefs.getString("endpoint","").orEmpty();val token=prefs.getString("token","").orEmpty()
        require(endpoint.isNotEmpty()&&token.isNotEmpty()){"Connect this tablet to the PC first."}
        val request=URI("$endpoint$path").toURL().openConnection()as HttpURLConnection
        try{
            request.requestMethod=method;request.connectTimeout=7000;request.readTimeout=7000;request.setRequestProperty("Authorization","Bearer $token")
            if(body!=null){request.doOutput=true;request.setRequestProperty("Content-Type","application/json");request.outputStream.use{it.write(body.toByteArray())}}
            val code=request.responseCode;val stream=if(code in 200..299)request.inputStream else request.errorStream
            return code to (stream?.bufferedReader()?.use{it.readText()}?:"")
        }finally{request.disconnect()}
    }
    private fun detail(text:String,fallback:String)=try{JSONObject(text).optString("detail",fallback)}catch(_:Exception){fallback}
    /** Uploads a script edited on the tablet; the PC's revision check rejects stale copies with 409. */
    private fun push(script:JSONObject){
        val id=script.getString("id")
        worker.execute{
            val event=try{
                val(code,text)=device("PUT","/api/device/scripts/"+java.net.URLEncoder.encode(id,"UTF-8"),script.toString())
                if(code in 200..299)JSONObject().put("type","push_ok").put("id",id).put("revision",JSONObject(text).getLong("revision"))
                else JSONObject().put("type","push_error").put("id",id).put("status",code).put("message",if(code==401)"Pairing expired · connect to PC again" else detail(text,"Could not sync with the PC."))
            }catch(e:Exception){JSONObject().put("type","push_error").put("id",id).put("message","Could not reach the PC. Edits stay on this tablet and sync later.")}
            emit(event)
        }
    }
    private fun deleteScript(id:String,revision:Long){
        worker.execute{
            val event=try{
                val(code,text)=device("DELETE","/api/device/scripts/"+java.net.URLEncoder.encode(id,"UTF-8")+"?revision=$revision",null)
                if(code in 200..299)JSONObject().put("type","delete_ok").put("id",id)
                else JSONObject().put("type","error").put("message",detail(text,"Could not delete on the PC."))
            }catch(e:Exception){JSONObject().put("type","error").put("message","Could not reach the PC.")}
            emit(event)
        }
    }
    /** Fetches the PC's library now, e.g. after an edit conflict. */
    private fun refresh(){
        worker.execute{try{val(code,text)=device("GET","/api/library",null);if(code in 200..299)saveLibrary(JSONObject(text))}catch(_:Exception){}}
    }
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
        if(checkSelfPermission(Manifest.permission.RECORD_AUDIO)!=PackageManager.PERMISSION_GRANTED){if(!permissionPrompt&&!cameraPrompt){permissionPrompt=true;requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO),101)};return}
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
    // ---- Camera: live preview behind the transparent WebView, recording to Movies/RustyPrompter ----
    // Rust decides when; this runs it. Camera state lives on `cameraThread`
    // (posted with onCamera); views change on the main thread.
    private var wantedCamera = false
    private var cameraFront = true
    private var cameraPrompt = false
    private var preview: TextureView? = null
    private var cameraThread: HandlerThread? = null
    private var cameraHandler: Handler? = null
    // Owned by cameraThread:
    private var cameraGeneration = 0
    private var cameraDevice: CameraDevice? = null
    private var cameraSession: CameraCaptureSession? = null
    private var previewSurface: Surface? = null
    @Volatile private var videoSize: Size? = null // also read by fitPreview
    @Volatile private var sensorOrientation = 0
    private var lensFront = true
    private var recorder: MediaRecorder? = null
    private var recordingUri: Uri? = null
    private var recordingFile: ParcelFileDescriptor? = null
    private var recordingName = ""
    private var sessionToken = 0
    private var recordingStarted = false
    private val recordingFolder = Environment.DIRECTORY_MOVIES + "/RustyPrompter"

    private fun cameraEvent(value: JSONObject) = emit(value.put("type", "camera"))
    private fun onCamera(block: () -> Unit) {
        if (cameraThread == null) { cameraThread = HandlerThread("camera").also { it.start() }; cameraHandler = Handler(cameraThread!!.looper) }
        cameraHandler?.post { try { block() } catch (e: Exception) { cameraFailed(e.message ?: "Camera problem") } }
    }
    /** Shows the camera behind the script. Main thread. */
    private fun cameraOpen(front: Boolean) {
        wantedCamera = true; cameraFront = front
        if (destroyed || !foreground) return
        val missing = listOf(Manifest.permission.CAMERA, Manifest.permission.RECORD_AUDIO).filter { checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }
        if (Manifest.permission.CAMERA in missing) { if (!permissionPrompt && !cameraPrompt) { cameraPrompt = true; requestPermissions(missing.toTypedArray(), 102) }; return }
        val view = preview ?: TextureView(this).also { created ->
            preview = created
            created.surfaceTextureListener = object : TextureView.SurfaceTextureListener {
                override fun onSurfaceTextureAvailable(texture: SurfaceTexture, width: Int, height: Int) { if (wantedCamera) startCamera(texture) }
                override fun onSurfaceTextureSizeChanged(texture: SurfaceTexture, width: Int, height: Int) = fitPreview()
                override fun onSurfaceTextureDestroyed(texture: SurfaceTexture): Boolean { onCamera { stopRecording(); closeDevice(); texture.release() }; return false } // released after the camera lets go
                override fun onSurfaceTextureUpdated(texture: SurfaceTexture) {}
            }
            created.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> fitPreview() }
            root.addView(created, 0, FrameLayout.LayoutParams(-1, -1))
            getSystemService(android.hardware.display.DisplayManager::class.java).registerDisplayListener(displayListener, main)
            web.setBackgroundColor(Color.TRANSPARENT)
        }
        val texture = view.surfaceTexture
        if (view.isAvailable && texture != null) startCamera(texture)
    }
    /** Turns the camera off for good (Rust asked). A recording is saved first. */
    private fun cameraClose() {
        wantedCamera = false
        onCamera { stopRecording(); closeDevice(); cameraEvent(JSONObject().put("state", "closed")) }
        removePreview()
    }
    /** The app went to the background: save, release the camera, reopen on resume. */
    private fun cameraSuspend() {
        onCamera { stopRecording(); closeDevice(); cameraEvent(JSONObject().put("state", "paused")) }
        removePreview()
    }
    private val displayListener = object : android.hardware.display.DisplayManager.DisplayListener {
        override fun onDisplayChanged(id: Int) { if (id == display?.displayId) fitPreview() } // 180° turns keep the view size
        override fun onDisplayAdded(id: Int) {}
        override fun onDisplayRemoved(id: Int) {}
    }
    /** Holds the screen still while recording, so the saved video stays upright. */
    private var orientationLocked = false // main thread
    private fun lockOrientation(lock: Boolean) = main.post {
        orientationLocked = lock
        if (lock) requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LOCKED
        else applyOrientation(prefs.getString("settings", "{}").orEmpty())
    }
    private fun removePreview() {
        if (preview != null) getSystemService(android.hardware.display.DisplayManager::class.java).unregisterDisplayListener(displayListener)
        preview?.let { root.removeView(it) }; preview = null
        web.setBackgroundColor(Color.rgb(16,24,23))
    }
    private fun cameraFailed(message: String) {
        stopRecording(); closeDevice()
        main.post { if (wantedCamera) { wantedCamera = false; removePreview() } }
        cameraEvent(JSONObject().put("state", "error").put("message", message))
    }
    private fun displayDegrees() = (display?.rotation ?: Surface.ROTATION_0) * 90
    private fun startCamera(texture: SurfaceTexture) {
        val front = cameraFront
        onCamera { openDevice(texture, front) }
    }
    /** Center-crops the upright camera image to fill the view. Main thread. */
    private fun fitPreview() {
        val view = preview ?: return
        val size = videoSize ?: return
        val w = view.width.toFloat(); val h = view.height.toFloat()
        if (w <= 0f || h <= 0f) return
        val cx = w / 2f; val cy = h / 2f
        // The texture is already upright for the device's natural orientation;
        // undo TextureView's stretch, turn it for the current rotation, then cover.
        val sideways = sensorOrientation % 180 != 0
        val bufW = if (sideways) size.height.toFloat() else size.width.toFloat()
        val bufH = if (sideways) size.width.toFloat() else size.height.toFloat()
        val degrees = displayDegrees()
        val matrix = Matrix()
        matrix.setRectToRect(RectF(0f, 0f, w, h), RectF(cx - bufW / 2f, cy - bufH / 2f, cx + bufW / 2f, cy + bufH / 2f), Matrix.ScaleToFit.FILL)
        matrix.postRotate(-degrees.toFloat(), cx, cy)
        val turned = degrees % 180 != 0
        val shownW = if (turned) bufH else bufW; val shownH = if (turned) bufW else bufH
        val scale = maxOf(w / shownW, h / shownH)
        matrix.postScale(scale, scale, cx, cy)
        view.setTransform(matrix)
    }
    /** Up to 1080p, preferring 16:9, that both the preview and the recorder can use. */
    private fun pickSize(map: android.hardware.camera2.params.StreamConfigurationMap): Size {
        val recordable = map.getOutputSizes(MediaRecorder::class.java).orEmpty().toSet()
        val both = map.getOutputSizes(SurfaceTexture::class.java).orEmpty().filter { it in recordable }.ifEmpty { recordable.toList() }
        val fits = both.filter { it.width <= 1920 && it.height <= 1080 }.ifEmpty { both }
        return fits.filter { it.width * 9 == it.height * 16 }.maxByOrNull { it.width * it.height }
            ?: fits.maxByOrNull { it.width * it.height } ?: Size(1280, 720)
    }
    @SuppressLint("MissingPermission")
    private fun openDevice(texture: SurfaceTexture, front: Boolean) {
        stopRecording(); closeDevice()
        if (checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) return
        val manager = getSystemService(CameraManager::class.java)
        val wanted = if (front) CameraCharacteristics.LENS_FACING_FRONT else CameraCharacteristics.LENS_FACING_BACK
        val ids = manager.cameraIdList
        val id = ids.firstOrNull { manager.getCameraCharacteristics(it).get(CameraCharacteristics.LENS_FACING) == wanted } ?: ids.firstOrNull()
        if (id == null) { cameraFailed("No camera found on this device."); return }
        val info = manager.getCameraCharacteristics(id)
        val map = info.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP) ?: run { cameraFailed("This camera can't record video."); return }
        val size = pickSize(map)
        videoSize = size
        sensorOrientation = info.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
        lensFront = info.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
        texture.setDefaultBufferSize(size.width, size.height)
        previewSurface = Surface(texture)
        main.post { fitPreview() }
        val generation = ++cameraGeneration
        manager.openCamera(id, object : CameraDevice.StateCallback() {
            override fun onOpened(device: CameraDevice) {
                if (generation != cameraGeneration) { device.close(); return }
                cameraDevice = device; startPreview()
                cameraEvent(JSONObject().put("state", "open").put("front", lensFront))
            }
            override fun onDisconnected(device: CameraDevice) { device.close(); if (generation == cameraGeneration) cameraFailed("The camera was taken by another app.") }
            override fun onError(device: CameraDevice, error: Int) { device.close(); if (generation == cameraGeneration) cameraFailed("Could not open the camera. Close other camera apps and try again.") }
        }, cameraHandler)
    }
    private fun session(surfaces: List<Surface>, template: Int, ready: () -> Unit) {
        val device = cameraDevice ?: return
        val generation = cameraGeneration
        val token = ++sessionToken // a newer session request makes older callbacks stale
        cameraSession?.close(); cameraSession = null
        val callback = object : CameraCaptureSession.StateCallback() {
            override fun onConfigured(created: CameraCaptureSession) {
                if (token != sessionToken || generation != cameraGeneration || cameraDevice !== device) { created.close(); return }
                cameraSession = created
                try {
                    val request = device.createCaptureRequest(template).apply {
                        surfaces.forEach { addTarget(it) }
                        set(CaptureRequest.CONTROL_MODE, CaptureRequest.CONTROL_MODE_AUTO)
                        fps30(device.id)?.let { set(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, it) }
                    }.build()
                    created.setRepeatingRequest(request, null, cameraHandler)
                    ready()
                } catch (e: Exception) { cameraFailed(e.message ?: "Camera problem") }
            }
            override fun onConfigureFailed(created: CameraCaptureSession) { if (token == sessionToken && generation == cameraGeneration) cameraFailed("This camera can't show and record at the same time.") }
        }
        val executor = java.util.concurrent.Executor { command -> cameraHandler?.post(command) }
        device.createCaptureSession(SessionConfiguration(SessionConfiguration.SESSION_REGULAR, surfaces.map { OutputConfiguration(it) }, executor, callback))
    }
    private fun fps30(id: String): Range<Int>? =
        getSystemService(CameraManager::class.java).getCameraCharacteristics(id)
            .get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES)
            ?.filter { it.upper == 30 }?.maxByOrNull { it.lower }
    private fun startPreview() { previewSurface?.let { session(listOf(it), CameraDevice.TEMPLATE_PREVIEW) {} } }
    @SuppressLint("MissingPermission") // RECORD_AUDIO is checked before the audio source is set
    private fun startRecording(name: String) {
        val device = cameraDevice; val surface = previewSurface; val size = videoSize
        if (device == null || surface == null || size == null || recorder != null) { cameraEvent(JSONObject().put("state", "stopped")); return }
        val values = ContentValues().apply {
            put(MediaStore.Video.Media.DISPLAY_NAME, "$name.mp4")
            put(MediaStore.Video.Media.MIME_TYPE, "video/mp4")
            put(MediaStore.Video.Media.RELATIVE_PATH, recordingFolder)
            put(MediaStore.Video.Media.IS_PENDING, 1)
        }
        val uri = contentResolver.insert(MediaStore.Video.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY), values)
            ?: run { cameraEvent(JSONObject().put("state", "error").put("message", "Could not create the video file. Is storage full?")); return }
        recordingUri = uri; recordingName = "$name.mp4"
        try {
            val file = contentResolver.openFileDescriptor(uri, "w") ?: throw IllegalStateException("Could not create the video file.")
            recordingFile = file
            val audio = checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
            val degrees = displayDegrees()
            val hint = if (lensFront) (sensorOrientation + degrees) % 360 else (sensorOrientation - degrees + 360) % 360
            val created = if (Build.VERSION.SDK_INT >= 31) MediaRecorder(this) else @Suppress("DEPRECATION") MediaRecorder()
            recorder = created
            created.apply {
                // CAMCORDER sits beside the voice tracker's VOICE_RECOGNITION input.
                if (audio) setAudioSource(MediaRecorder.AudioSource.CAMCORDER)
                setVideoSource(MediaRecorder.VideoSource.SURFACE)
                setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
                setOutputFile(file.fileDescriptor)
                setVideoEncodingBitRate(if (size.height >= 1080) 12_000_000 else 8_000_000)
                setVideoFrameRate(30)
                setVideoSize(size.width, size.height)
                setVideoEncoder(MediaRecorder.VideoEncoder.H264)
                if (audio) { setAudioEncoder(MediaRecorder.AudioEncoder.AAC); setAudioEncodingBitRate(192_000); setAudioSamplingRate(48_000); setAudioChannels(1) }
                setOrientationHint(hint)
                prepare()
            }
            session(listOf(surface, created.surface), CameraDevice.TEMPLATE_RECORD) {
                try { created.start(); recordingStarted = true; lockOrientation(true); cameraEvent(JSONObject().put("state", "recording")) }
                catch (e: Exception) { discardRecording(); startPreview(); cameraEvent(JSONObject().put("state", "error").put("message", "Could not start recording.")) }
            }
        } catch (e: Exception) {
            discardRecording(); startPreview()
            cameraEvent(JSONObject().put("state", "error").put("message", "Could not start recording. ${e.message ?: ""}".trim()))
        }
    }
    /** Stops and publishes the recording to the gallery. No-op when not recording. */
    private fun stopRecording() {
        val active = recorder ?: return
        val started = recordingStarted
        recordingStarted = false; lockOrientation(false)
        if (!started) { sessionToken++; discardRecording(); cameraEvent(JSONObject().put("state", "stopped")); return } // its session is now stale
        try { cameraSession?.stopRepeating() } catch (_: Exception) {}
        val kept = try { active.stop(); true } catch (_: RuntimeException) { false } // stopped before any frame
        if (!kept) { discardRecording(); cameraEvent(JSONObject().put("state", "stopped")); return }
        active.release(); recorder = null
        try { recordingFile?.close() } catch (_: Exception) {}; recordingFile = null
        recordingUri?.let { uri ->
            contentResolver.update(uri, ContentValues().apply { put(MediaStore.Video.Media.IS_PENDING, 0) }, null, null)
            // MediaStore renames on a clash; report the name actually saved.
            try { contentResolver.query(uri, arrayOf(MediaStore.Video.Media.DISPLAY_NAME), null, null, null)?.use { if (it.moveToFirst()) recordingName = it.getString(0) ?: recordingName } } catch (_: Exception) {}
        }
        recordingUri = null
        cameraEvent(JSONObject().put("state", "saved").put("folder", "Gallery › $recordingFolder").put("file", recordingName))
    }
    private fun discardRecording() {
        try { recorder?.reset(); recorder?.release() } catch (_: Exception) {}; recorder = null
        try { recordingFile?.close() } catch (_: Exception) {}; recordingFile = null
        recordingUri?.let { try { contentResolver.delete(it, null, null) } catch (_: Exception) {} }; recordingUri = null
    }
    private fun closeDevice() {
        cameraGeneration++
        try { cameraSession?.close() } catch (_: Exception) {}; cameraSession = null
        try { cameraDevice?.close() } catch (_: Exception) {}; cameraDevice = null
        previewSurface?.release(); previewSurface = null
    }
    override fun onResume(){super.onResume();foreground=true;if(wantedAudio)startAudio();if(wantedCamera&&!cameraPrompt)cameraOpen(cameraFront)}
    override fun onPause(){foreground=false;stopAudio(!(permissionPrompt||cameraPrompt));emit(JSONObject().put("type","pause"));if(wantedCamera&&!cameraPrompt)cameraSuspend();super.onPause()}
    override fun onRequestPermissionsResult(code:Int,permissions:Array<out String>,results:IntArray){super.onRequestPermissionsResult(code,permissions,results);if(code==102){cameraPrompt=false;if(results.isEmpty()){if(wantedCamera&&foreground)main.post{cameraOpen(cameraFront)};return};if(wantedAudio&&foreground)startAudio();if(permissions.indexOf(Manifest.permission.CAMERA).let{it>=0&&results[it]==PackageManager.PERMISSION_GRANTED}){if(wantedCamera)cameraOpen(cameraFront)}else{wantedCamera=false;cameraEvent(JSONObject().put("state","error").put("message","Allow camera access in Android settings to record yourself."))}};if(code==101){permissionPrompt=false;if(results.firstOrNull()==PackageManager.PERMISSION_GRANTED){wantedAudio=true;emit(JSONObject().put("type","permission_granted"));startAudio()}else{stopAudio();emit(JSONObject().put("type","audio_error").put("message","Allow microphone access in Android settings to follow your voice. Manual scrolling and Next still work."))}}}
    override fun onDestroy(){destroyed=true;epoch++;socket?.close();stopAudio();wantedCamera=false;onCamera{stopRecording();closeDevice()};cameraThread?.quitSafely();val loaded=model;model=null;worker.shutdown();audioWorker.execute{loaded?.close()};audioWorker.shutdown();if(::web.isInitialized){web.removeJavascriptInterface("RustyNative");web.destroy()};super.onDestroy()}
}
