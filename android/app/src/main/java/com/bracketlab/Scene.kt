package com.bracketlab

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Typeface
import android.text.TextPaint
import android.text.TextUtils
import android.graphics.BitmapFactory
import android.util.Base64
import java.io.OutputStream
import kotlin.math.ceil

const val CARD_WIDTH = 354f
private const val CARD_HEIGHT = 132f
const val DARK = 0xff050b12.toInt()
const val PANEL = 0xff08111b.toInt()
const val CARD = 0xff11212e.toInt()
const val TEXT = 0xfff1f7fb.toInt()
const val MUTED = 0xff9caebb.toInt()
const val AMBER = 0xffffb02e.toInt()
const val DIVIDER = 0xff243744.toInt()

val PODIUM_COLORS = listOf(AMBER,0xffbecbd9.toInt(),0xffd29a78.toInt())

data class CardGeometry(val id: Int, val rect: RectF)
data class Link(val source: Int, val target: Int, val loser: Boolean, val path: Path, val bounds: RectF)
data class Heading(val label: String, val x: Float, val y: Float)

class BracketScene(val stage: Stage, slice: ExportSlice = ExportSlice.All) {
    val cards: List<CardGeometry>
    val links: List<Link>
    val headings: List<Heading>
    val bounds: RectF
    val width: Float
    val height: Float

    init {
        val nodes = mutableListOf<CardGeometry>()
        val titles = mutableListOf<Heading>()
        val lanes = mutableMapOf<String,Float>()
        var sectionY = 16f
        for ((lane, indexes) in stage.included(slice).groupBy { stage.matches[it].lane }) {
            lanes[lane] = sectionY
            titles += Heading(lane.uppercase(), 0f, sectionY)
            sectionY += 58
            val rounds = indexes.groupBy { stage.matches[it].round }.toSortedMap()
            val rows = rounds.values.maxOf { it.size }
            fun height(id: Int): Float {
                val count=stage.matches[id].mapRows().size
                return if (count == 0) CARD_HEIGHT else 138f + 23f * count
            }
            val pitch = indexes.maxOf(::height) + 30f
            for ((column, entry) in rounds.entries.withIndex()) {
                val x = column * 436f
                titles += Heading("РАУНД %02d".format(entry.key + 1), x, sectionY - 14)
                entry.value.forEachIndexed { row, id ->
                    val y = sectionY + ((row + 0.5f) * rows / entry.value.size) * pitch - height(id) / 2
                    nodes += CardGeometry(id, RectF(x, y, x + CARD_WIDTH, y + height(id)))
                }
            }
            sectionY += rows * pitch + 68
        }
        cards = nodes.sortedBy(CardGeometry::id)
        headings = titles.toList()
        val byId = cards.associateBy { it.id }
        links = stage.matches.flatMapIndexed { target, game ->
            game.sources.mapIndexedNotNull { slot, source ->
                if (source.type != "winner" && source.type != "loser") return@mapIndexedNotNull null
                val index = source.index()
                val a = byId[index]?.rect ?: return@mapIndexedNotNull null
                val b = byId[target]?.rect ?: return@mapIndexedNotNull null
                val start = a.top + 77
                val end = b.top + 59 + 36*slot
                if (stage.matches[index].lane != game.lane) {
                    val route = lanes.getValue(game.lane)-18
                    val spread = (index % 7)*5
                    val out = a.right+13+spread
                    val into = b.left-13-spread
                    val path = Path().apply {
                        moveTo(a.right,start); lineTo(out,start); lineTo(out,route)
                        lineTo(into,route); lineTo(into,end); lineTo(b.left,end)
                    }
                    val bounds = RectF(minOf(a.right,out,into,b.left),minOf(start,route,end),
                        maxOf(a.right,out,into,b.left),maxOf(start,route,end)).apply { inset(-2f,-2f) }
                    return@mapIndexedNotNull Link(index,target,source.type=="loser",path,bounds)
                }
                if (a.right >= b.left) return@mapIndexedNotNull null
                val middle = (a.right+b.left)/2
                val path = Path().apply { moveTo(a.right,start); cubicTo(middle,start,middle,end,b.left,end) }
                Link(index, target, source.type == "loser", path, RectF(a).apply { union(b) })
            }
        }
        bounds = RectF(0f,0f,1f,1f).apply {
            cards.forEach { union(it.rect) }
            links.forEach { union(it.bounds) }
        }
        width=bounds.width()
        height=bounds.height()
    }
}

class ScenePainter(project: Project, private val geometry: BracketScene, private val font: Typeface,
    private val logos: Map<String,Bitmap> = decodeLogos(project.teams)) {
    private val paint = TextPaint(Paint.ANTI_ALIAS_FLAG)
    private val stage = geometry.stage
    private val states = stage.resolve()
    private val names = project.teams.associate { it.id to it.name }


    private fun caption(canvas: Canvas, text: String, x: Float, y: Float, size: Float, color: Int, maxWidth: Float = Float.POSITIVE_INFINITY, mono: Boolean = false) {
        paint.style = Paint.Style.FILL
        paint.typeface = if (mono) Typeface.MONOSPACE else font
        paint.textSize = size
        paint.color = color
        val fitted = if (maxWidth.isFinite()) TextUtils.ellipsize(text, paint, maxWidth, TextUtils.TruncateAt.END).toString() else text
        canvas.drawText(fitted, x, y, paint)
    }

    fun paint(canvas: Canvas, viewport: RectF, zoom: Float, selected: Int? = null, round: Int? = null,
        affected: Set<Int> = emptySet(), pulse: Float = 0f) {
        if (round == null) {
            geometry.headings.forEach { caption(canvas, it.label, it.x, it.y, 13f, MUTED) }
            paint.style = Paint.Style.STROKE
            paint.strokeWidth = 1.8f
            for (edge in geometry.links) {
                if (!RectF.intersects(edge.bounds, viewport)) continue
                paint.color = when {
                    selected == edge.source || selected == edge.target -> AMBER
                    edge.loser -> DIVIDER
                    states[edge.source].result != null -> 0xff936931.toInt()
                    else -> DIVIDER
                }
                canvas.drawPath(edge.path, paint)
            }
        }
        for ((id, rect) in geometry.cards) {
            val game = stage.matches[id]
            if (!RectF.intersects(rect, viewport) || round != null && game.round != round) continue
            paint.color = CARD
            paint.style = Paint.Style.FILL
            canvas.drawRoundRect(rect, 14f, 14f, paint)
            if (id in affected && pulse > 0f) {
                paint.color = AMBER
                paint.alpha = (pulse * 45).toInt()
                canvas.drawRoundRect(rect, 14f, 14f, paint)
            }
            paint.color = if (id == selected) AMBER else DIVIDER
            paint.style = Paint.Style.STROKE
            canvas.drawRoundRect(rect, 14f, 14f, paint)
            val state = states[id]
            val status = when {
                !state.active -> "УСЛОВНЫЙ"
                game.score?.technical == true -> "TECH"
                game.score != null && state.result == null -> "ИДЁТ"
                game.score != null -> "ЗАВЕРШЁН"
                state.result != null -> "BYE"
                state.ready -> "ГОТОВ"
                else -> "ОЖИДАНИЕ"
            }
            caption(canvas, "M%02d  /  BO%d  /  %s".format(id + 1, game.bo, status), rect.left + 14, rect.top + 22, 12f, MUTED)
            if (zoom < 0.42f) continue
            paint.color = DIVIDER
            paint.strokeWidth = 1f
            canvas.drawLine(rect.left + 14, rect.top + 35, rect.right - 14, rect.top + 35, paint)
            repeat(2) { slot ->
                val team = state.teams[slot]
                val source = game.sources[slot]
                val label = when (team) {
                    "" -> "BYE"
                    null -> when (source.type) {
                        "winner" -> "Победитель M%02d".format(source.index() + 1)
                        "loser" -> "Проигравший M%02d".format(source.index() + 1)
                        else -> "Ожидается"
                    }
                    else -> names[team] ?: "Команда удалена"
                }
                val color = when {
                    !team.isNullOrEmpty() && state.result?.first() == team -> AMBER
                    team.isNullOrEmpty() -> MUTED
                    else -> TEXT
                }
                val y = rect.top + 64 + slot * 36
                var x = rect.left + 14
                if (!team.isNullOrEmpty()) {
                    val avatar = RectF(x, y - 16, x + 22, y + 6)
                    val logo = logos[team]
                    paint.style = Paint.Style.FILL
                    if (logo == null) {
                        paint.color = 0xff172c3a.toInt()
                        canvas.drawRoundRect(avatar, 5f, 5f, paint)
                        caption(canvas, label.take(1).uppercase(), x + 7, y, 11f, MUTED)
                    } else {
                        val fit = 22f / maxOf(logo.width, logo.height)
                        val halfWidth = logo.width * fit / 2
                        val halfHeight = logo.height * fit / 2
                        avatar.set(avatar.centerX() - halfWidth, avatar.centerY() - halfHeight,
                            avatar.centerX() + halfWidth, avatar.centerY() + halfHeight)
                        paint.isFilterBitmap = true
                        canvas.drawBitmap(logo, null, avatar, paint)
                    }
                    x += 32
                }
                caption(canvas, label, x, y, 15f, color, rect.right - x - 46)
                game.score?.let { rightCaption(canvas,it.series[slot].toString(),rect.right-14,y,20f,color) }
            }
            val rows = if (zoom >= 0.65f) game.mapRows() else emptyList()
            if (rows.isNotEmpty()) {
                val alpha = ((zoom-0.65f)/0.10f).coerceIn(0f,1f)
                fun fade(color: Int) = (color and 0xffffff) or ((alpha*255).toInt() shl 24)
                paint.color = fade(DIVIDER)
                canvas.drawLine(rect.left+14,rect.top+117,rect.right-14,rect.top+117,paint)
                rows.forEachIndexed { row,(name,score) ->
                    val y=rect.top+139+23*row
                    caption(canvas,name,rect.left+14,y,13f,fade(MUTED),rect.width()-120)
                    rightCaption(canvas,score,rect.right-14,y,14f,fade(TEXT))
                }
            }
        }
    }

    private fun rightCaption(canvas: Canvas, text: String, x: Float, y: Float, size: Float, color: Int) {
        paint.textAlign = Paint.Align.RIGHT
        caption(canvas,text,x,y,size,color,mono=true)
        paint.textAlign = Paint.Align.LEFT
    }

    companion object {
        fun decodeLogos(teams: List<Team>): Map<String,Bitmap> = teams.mapNotNull { team ->
            val logo = team.logoPng ?: return@mapNotNull null
            val bitmap = runCatching {
                val png = Base64.decode(logo, Base64.DEFAULT)
                val limits = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(png, 0, png.size, limits)
                require(limits.outWidth in 1..512 && limits.outHeight in 1..512)
                BitmapFactory.decodeByteArray(png, 0, png.size)
            }.getOrNull()
            bitmap?.let { team.id to it }
        }.toMap()

        fun writePng(project: Project, selection: List<Pair<Int, ExportSlice>>, scale: Int, margin: Int,
            transparent: Boolean, podium: Boolean, destination: OutputStream, font: Typeface) {
            val scenes = selection.map { (index,slice) -> BracketScene(project.stages[index],slice) to slice }
            val width = ceil(maxOf(568f + 2 * margin, scenes.maxOf { it.first.width + 2 * margin }) * scale).toInt()
            val height = ceil((margin + 62f + scenes.sumOf { it.first.height.toDouble() + margin + 58 } + if (podium) 132 + margin else 0) * scale).toInt()
            require(width <= 16384 && height <= 16384 && width.toLong() * height <= 16_000_000) {
                "Изображение слишком велико. Выберите раунд, этап или меньший масштаб."
            }
            val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
            try {
                val canvas = Canvas(bitmap)
                if (!transparent) canvas.drawColor(DARK)
                canvas.scale(scale.toFloat(), scale.toFloat())
                val painter = ScenePainter(project, scenes.first().first, font)
                painter.caption(canvas, project.name, margin.toFloat(), margin + 12f, 27f, Color.WHITE, 550f)
                var y = margin + 62f
                for ((scene,slice) in scenes) {
                    val bounds=scene.bounds
                    val title = scene.stage.name + when (slice) {
                        ExportSlice.All -> ""
                        is ExportSlice.Round -> " / Раунд ${slice.round+1}"
                        is ExportSlice.FromMatch -> " / С матча M%02d".format(slice.index+1)
                    }
                    painter.caption(canvas, title, margin.toFloat(), y, 19f, AMBER, 550f)
                    canvas.save()
                    canvas.translate((width.toFloat()/scale-bounds.width())/2 - bounds.left, y + 30f - bounds.top)
                    ScenePainter(project, scene, font, painter.logos).paint(canvas, bounds, 1f)
                    canvas.restore()
                    y += bounds.height() + margin + 58f
                }
                if (podium) {
                    painter.caption(canvas,"ПРИЗОВЫЕ МЕСТА",margin.toFloat(),y,19f,AMBER)
                    project.podium().forEachIndexed { i,id ->
                        val baseline=y+32+i*38
                        painter.caption(canvas,"${i+1} место",margin.toFloat(),baseline,15f,PODIUM_COLORS[i])
                        painter.logos[id]?.let { logo ->
                            val h = 24f * logo.height/maxOf(logo.width,logo.height)
                            val w = 24f * logo.width/maxOf(logo.width,logo.height)
                            canvas.drawBitmap(logo,null,RectF(margin+104-w/2,baseline-6-h/2,margin+104+w/2,baseline-6+h/2),null)
                        }
                        painter.caption(canvas,id?.let(project::teamName) ?: "Место пока не определено",margin+126f,baseline,16f,TEXT,410f)
                    }
                }
                check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, destination)) { "Не удалось сохранить PNG" }
            } finally {
                bitmap.recycle()
            }
        }
    }
}
