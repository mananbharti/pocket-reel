// Copied into the generated Android project by scripts/patch-android.py.
//
// Downloads from YouTube and other sites with yt-dlp, using youtubedl-android
// (the library YTDLnis builds on). It also receives links shared to the app
// from other apps (YouTube's Share button, browsers).

package app.pocketreel.converter

import android.app.Activity
import android.content.Intent
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLRequest
import java.io.File
import java.util.concurrent.ConcurrentHashMap

@InvokeArg
class UrlArgs {
    lateinit var url: String
}

@InvokeArg
class DownloadArgs {
    lateinit var url: String
    lateinit var dir: String
    lateinit var id: String
}

@InvokeArg
class IdArgs {
    lateinit var id: String
}

@TauriPlugin
class YtdlPlugin(private val activity: Activity) : Plugin(activity) {
    @Volatile private var pendingShare: String? = null
    @Volatile private var ready = false
    private val progress = ConcurrentHashMap<String, Pair<Float, Long>>()

    override fun load(webView: WebView) {
        super.load(webView)
        handleIntent(activity.intent)
    }

    override fun onNewIntent(intent: Intent) {
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        if (intent == null) return
        val text = when (intent.action) {
            Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)
            Intent.ACTION_VIEW -> intent.dataString
            else -> null
        } ?: return
        val url = Regex("https?://\\S+").find(text)?.value ?: return
        pendingShare = url
        val event = JSObject()
        event.put("url", url)
        trigger("share", event)
    }

    @Synchronized
    private fun ensureReady() {
        if (!ready) {
            YoutubeDL.getInstance().init(activity.applicationContext)
            ready = true
        }
    }

    @Command
    fun takeShared(invoke: Invoke) {
        val r = JSObject()
        r.put("url", pendingShare)
        pendingShare = null
        invoke.resolve(r)
    }

    @Command
    fun info(invoke: Invoke) {
        val args = invoke.parseArgs(UrlArgs::class.java)
        Thread {
            try {
                ensureReady()
                val request = YoutubeDLRequest(args.url)
                request.addOption("--no-playlist")
                val info = YoutubeDL.getInstance().getInfo(request)
                val r = JSObject()
                r.put("id", info.id ?: "")
                r.put("title", info.title ?: "Video")
                r.put("duration", info.duration)
                r.put("thumbnail", info.thumbnail)
                r.put("uploader", info.uploader)
                r.put("url", info.webpageUrl ?: args.url)
                invoke.resolve(r)
            } catch (e: Throwable) {
                invoke.reject(e.message ?: "Couldn't read this link")
            }
        }.start()
    }

    @Command
    fun download(invoke: Invoke) {
        val args = invoke.parseArgs(DownloadArgs::class.java)
        Thread {
            try {
                ensureReady()
                val dir = File(args.dir)
                dir.mkdirs()
                val request = YoutubeDLRequest(args.url)
                request.addOption("--no-playlist")
                request.addOption("-f", FORMAT)
                request.addOption("--merge-output-format", "mp4")
                request.addOption("--no-mtime")
                request.addOption("-o", File(dir, "%(id)s.%(ext)s").absolutePath)
                request.addOption("--print", "after_move:FILE %(filepath)s")
                request.addOption("--progress")
                request.addOption("--newline")
                progress[args.id] = Pair(0f, -1L)
                val response = YoutubeDL.getInstance().execute(request, args.id) { p, eta, _ ->
                    progress[args.id] = Pair(p, eta)
                }
                val path = response.out.lines().lastOrNull { it.startsWith("FILE ") }?.removePrefix("FILE ")?.trim()
                    ?: dir.listFiles()?.filter { it.isFile }?.maxByOrNull { it.lastModified() }?.absolutePath
                val r = JSObject()
                r.put("path", path)
                invoke.resolve(r)
            } catch (e: YoutubeDL.CanceledException) {
                invoke.reject("Canceled")
            } catch (e: Throwable) {
                invoke.reject(e.message ?: "The download failed")
            } finally {
                progress.remove(args.id)
            }
        }.start()
    }

    @Command
    fun progress(invoke: Invoke) {
        val id = invoke.parseArgs(IdArgs::class.java).id
        val p = progress[id]
        val r = JSObject()
        r.put("progress", (p?.first ?: 0f).toDouble())
        r.put("eta", p?.second ?: -1L)
        r.put("active", p != null)
        invoke.resolve(r)
    }

    @Command
    fun cancel(invoke: Invoke) {
        val id = invoke.parseArgs(IdArgs::class.java).id
        YoutubeDL.getInstance().destroyProcessById(id)
        invoke.resolve()
    }

    @Command
    fun update(invoke: Invoke) {
        Thread {
            try {
                ensureReady()
                val status = YoutubeDL.getInstance().updateYoutubeDL(activity.applicationContext)
                val r = JSObject()
                r.put("status", status?.name ?: "UNKNOWN")
                invoke.resolve(r)
            } catch (e: Throwable) {
                invoke.reject(e.message ?: "Update failed")
            }
        }.start()
    }

    companion object {
        // Same choice as desktop: small H.264 + AAC is plenty for a 160 x 120 player.
        const val FORMAT = "bv*[height<=360][vcodec^=avc1]+ba[ext=m4a]/bv*[height<=480]+ba/b[height<=480]/b"
    }
}
