// Copied into the generated Android project by scripts/patch-android.py.
//
// Downloads from YouTube and other sites with yt-dlp, using youtubedl-android
// (the library YTDLnis builds on). It also receives links shared to the app
// from other apps (YouTube's Share button, browsers).

package app.pocketreel.converter

import android.app.Activity
import android.content.Intent
import android.os.Build
import androidx.core.content.FileProvider
import android.util.Log
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
import java.net.HttpURLConnection
import java.net.URL
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
    var format: String? = null
    var ffmpeg: String? = null
}

@InvokeArg
class FetchArgs {
    lateinit var url: String
    var path: String? = null
    var id: String? = null
}

@InvokeArg
class PathArgs {
    lateinit var path: String
}

/** Shares a downloaded update with the system installer (see update_paths.xml). */
class UpdateFileProvider : FileProvider()

@InvokeArg
class IdArgs {
    lateinit var id: String
}

@TauriPlugin
class YtdlPlugin(private val activity: Activity) : Plugin(activity) {
    @Volatile private var pendingShare: String? = null
    @Volatile private var ready = false
    private val progress = ConcurrentHashMap<String, Triple<Float, Long, String>>()

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
                // yt-dlp's JSON is parsed in Rust, the same way as on desktop.
                val request = YoutubeDLRequest(args.url)
                request.addOption("--no-playlist")
                request.addOption("--skip-download")
                request.addOption("--dump-single-json")
                val response = YoutubeDL.getInstance().execute(request)
                val r = JSObject()
                r.put("json", response.out)
                invoke.resolve(r)
            } catch (e: Throwable) {
                Log.e(TAG, "info failed for ${args.url}", e)
                invoke.reject(describe(e, "Couldn't read this link"))
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
                request.addOption("-f", args.format ?: FORMAT)
                request.addOption("--merge-output-format", "mp4")
                // Without FFmpeg yt-dlp leaves separate picture and sound files.
                args.ffmpeg?.takeIf { File(it).isFile }?.let { request.addOption("--ffmpeg-location", it) }
                request.addOption("--no-mtime")
                request.addOption("-o", File(dir, "%(id)s.%(ext)s").absolutePath)
                request.addOption("--print", "after_move:FILE %(filepath)s")
                request.addOption("--progress")
                request.addOption("--newline")
                progress[args.id] = Triple(0f, -1L, "")
                val response = YoutubeDL.getInstance().execute(request, args.id) { p, eta, line ->
                    progress[args.id] = Triple(p, eta, line)
                }
                val printed = response.out.lines().lastOrNull { it.startsWith("FILE ") }?.removePrefix("FILE ")?.trim()
                val files = dir.listFiles()?.filter { it.isFile && !it.name.endsWith(".part") }.orEmpty()
                // Several files left over means picture and sound weren't joined;
                // converting just one of them would lose the sound.
                if (printed?.let { File(it).isFile } != true && files.size > 1) {
                    Log.e(TAG, "yt-dlp left ${files.map { it.name }} unmerged. stderr: ${response.err}")
                    diagnoseFfmpeg()
                    invoke.reject("Couldn't join this video's picture and sound")
                    return@Thread
                }
                val path = printed?.takeIf { File(it).isFile } ?: files.firstOrNull()?.absolutePath
                val r = JSObject()
                r.put("path", path)
                invoke.resolve(r)
            } catch (e: YoutubeDL.CanceledException) {
                invoke.reject("Canceled")
            } catch (e: Throwable) {
                Log.e(TAG, "download failed for ${args.url}", e)
                diagnoseFfmpeg()
                invoke.reject(describe(e, "The download failed"))
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
        r.put("line", p?.third ?: "")
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
                Log.e(TAG, "update failed", e)
                invoke.reject(describe(e, "Update failed"))
            }
        }.start()
    }

    // ------------------------------------------------------------ updates

    private val fetches = ConcurrentHashMap<String, Pair<Long, Long>>()

    private fun open(url: String): HttpURLConnection {
        val c = URL(url).openConnection() as HttpURLConnection
        c.instanceFollowRedirects = true
        c.connectTimeout = 15000
        c.readTimeout = 60000
        c.setRequestProperty("User-Agent", "PocketReel-updater")
        c.setRequestProperty("Accept", "application/vnd.github+json, */*")
        if (c.responseCode !in 200..299) {
            val code = c.responseCode
            c.disconnect()
            throw RuntimeException("The update server answered with error $code.")
        }
        return c
    }

    @Command
    fun fetchText(invoke: Invoke) {
        val args = invoke.parseArgs(FetchArgs::class.java)
        Thread {
            try {
                val c = open(args.url)
                val text = c.inputStream.bufferedReader().use { it.readText() }
                c.disconnect()
                val r = JSObject()
                r.put("text", text)
                invoke.resolve(r)
            } catch (e: Throwable) {
                Log.w(TAG, "fetch failed for ${args.url}", e)
                invoke.reject(describe(e, "Couldn't reach the update server"))
            }
        }.start()
    }

    @Command
    fun fetchFile(invoke: Invoke) {
        val args = invoke.parseArgs(FetchArgs::class.java)
        val id = args.id ?: "update"
        Thread {
            val dest = File(args.path ?: return@Thread invoke.reject("No path"))
            val tmp = File(dest.path + ".part")
            try {
                dest.parentFile?.mkdirs()
                val c = open(args.url)
                val total = c.contentLengthLong
                fetches[id] = Pair(0L, total)
                var done = 0L
                c.inputStream.use { input ->
                    tmp.outputStream().use { out ->
                        val buf = ByteArray(1 shl 16)
                        while (true) {
                            val n = input.read(buf)
                            if (n < 0) break
                            out.write(buf, 0, n)
                            done += n
                            fetches[id] = Pair(done, total)
                        }
                    }
                }
                c.disconnect()
                if (total > 0 && done != total) throw RuntimeException("The update download was cut short. Try again.")
                if (!tmp.renameTo(dest)) throw RuntimeException("Couldn't save the update")
                invoke.resolve()
            } catch (e: Throwable) {
                tmp.delete()
                Log.w(TAG, "download failed for ${args.url}", e)
                invoke.reject(describe(e, "The update download failed"))
            } finally {
                fetches.remove(id)
            }
        }.start()
    }

    @Command
    fun fetchProgress(invoke: Invoke) {
        val id = invoke.parseArgs(FetchArgs::class.java).id ?: "update"
        val p = fetches[id]
        val r = JSObject()
        r.put("done", p?.first ?: 0L)
        r.put("total", p?.second ?: -1L)
        invoke.resolve(r)
    }

    /** Opens Android's installer for a downloaded APK (signed with the same key). */
    @Command
    fun installApk(invoke: Invoke) {
        val path = invoke.parseArgs(PathArgs::class.java).path
        try {
            val ctx = activity.applicationContext
            val uri = FileProvider.getUriForFile(ctx, ctx.packageName + ".updates", File(path))
            val intent = Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
            val r = JSObject()
            // Android 8+ asks once to allow installing apps from Pocket Reel.
            r.put("needsPermission", Build.VERSION.SDK_INT >= 26 && !ctx.packageManager.canRequestPackageInstalls())
            invoke.resolve(r)
        } catch (e: Throwable) {
            Log.e(TAG, "install failed for $path", e)
            invoke.reject(describe(e, "Couldn't open the installer"))
        }
    }

    /** Logs whether yt-dlp can run the bundled FFmpeg (it joins picture and sound). */
    private fun diagnoseFfmpeg() {
        try {
            val ctx = activity.applicationContext
            val ff = File(ctx.applicationInfo.nativeLibraryDir, "libffmpeg.so")
            val lib = File(ctx.noBackupFilesDir, "youtubedl-android/packages")
            fun run(env: Map<String, String>): String = try {
                val pb = ProcessBuilder(ff.absolutePath, "-hide_banner", "-bsfs").redirectErrorStream(true)
                pb.environment().putAll(env)
                val proc = pb.start()
                proc.outputStream.close()
                val out = proc.inputStream.bufferedReader().readText()
                "exit=${proc.waitFor()} out=${out.take(300).replace('\n', ' ')}"
            } catch (t: Throwable) {
                "failed: $t"
            }
            val ld = listOf("python", "ffmpeg", "aria2c").joinToString(":") { "$lib/$it/usr/lib" }
            Log.w(TAG, "ffmpeg check: exists=${ff.exists()} canExecute=${ff.canExecute()} size=${ff.length()}")
            Log.w(TAG, "ffmpeg plain: ${run(emptyMap())}")
            Log.w(TAG, "ffmpeg ytdl-env: ${run(mapOf("LD_LIBRARY_PATH" to ld))}")
            val request = YoutubeDLRequest(emptyList())
            request.addOption("--verbose")
            request.addOption("--no-update")
            try {
                YoutubeDL.getInstance().execute(request)
            } catch (t: Throwable) {
                val debug = (t.message ?: "").lines().filter { "exe versions" in it || "Python" in it || "ffmpeg" in it.lowercase() }
                Log.w(TAG, "yt-dlp view: ${debug.joinToString(" | ")}")
            }
        } catch (t: Throwable) {
            Log.w(TAG, "ffmpeg check failed", t)
        }
    }

    /** The error text plus its causes, so the real reason reaches the app. */
    private fun describe(e: Throwable, fallback: String): String {
        val parts = generateSequence(e) { it.cause }.take(4)
            .mapNotNull { it.message?.trim()?.takeIf { m -> m.isNotEmpty() } ?: it.javaClass.simpleName }
            .distinct().toList()
        return if (parts.isEmpty()) fallback else parts.joinToString(": ")
    }

    companion object {
        const val TAG = "PocketReel"
        // Same choice as desktop: small H.264 + AAC is plenty for a 160 x 120 player.
        const val FORMAT = "bv*[height<=360][vcodec^=avc1]+ba[ext=m4a]/bv*[height<=480]+ba/b[height<=480]/b"
    }
}
