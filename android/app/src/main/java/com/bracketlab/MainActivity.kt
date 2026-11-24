package com.bracketlab

import android.graphics.RectF
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.Base64
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.layout.ContentScale
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.core.content.res.ResourcesCompat
import coil3.compose.AsyncImage
import java.io.ByteArrayOutputStream
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private val inter = FontFamily(Font(R.font.inter_regular), Font(R.font.inter_semibold, FontWeight.SemiBold))

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme(
                background = Color(DARK), surface = Color(PANEL), surfaceContainer = Color(CARD),
                primary = Color(AMBER), onPrimary = Color(DARK), onSurface = Color(TEXT),
                primaryContainer = Color(0xff312518), onPrimaryContainer = Color(AMBER),
                secondary = Color(AMBER), secondaryContainer = Color(0xff312518),
                onSecondaryContainer = Color(AMBER), surfaceVariant = Color(CARD),
                outline = Color(MUTED), outlineVariant = Color(DIVIDER)
            ), typography = Typography(
                bodyLarge = MaterialTheme.typography.bodyLarge.copy(fontFamily = inter),
                bodyMedium = MaterialTheme.typography.bodyMedium.copy(fontFamily = inter),
                titleLarge = MaterialTheme.typography.titleLarge.copy(fontFamily = inter),
                labelLarge = MaterialTheme.typography.labelLarge.copy(fontFamily = inter),
            )) { TournamentScreen() }
        }
    }
}

@Composable
private fun TournamentScreen(model: TournamentViewModel = viewModel()) {
    val screen by model.state.collectAsStateWithLifecycle()
    val reduceMotion by model.reduceMotion.collectAsStateWithLifecycle()
    val project = screen.project
    val context = LocalContext.current
    val resolver = context.contentResolver
    val font = remember(context) { ResourcesCompat.getFont(context, R.font.inter_regular) ?: android.graphics.Typeface.DEFAULT }
    var selectedStage by rememberSaveable(screen.tournamentId) { mutableIntStateOf(0) }
    val stageIndex = selectedStage.coerceAtMost((project.stages.size - 1).coerceAtLeast(0))
    val stage = project.stages.getOrNull(stageIndex)
    var selectedMatch by remember(screen.tournamentId, stageIndex, project.stages.size) { mutableStateOf<Int?>(null) }
    var editMatch by remember(screen.tournamentId, stageIndex, project.stages.size) { mutableStateOf(false) }
    var manager by rememberSaveable { mutableStateOf(false) }
    var podiumOpen by rememberSaveable { mutableStateOf(false) }
    var deleteStage by remember { mutableStateOf(false) }
    var roundFilter by remember(screen.tournamentId,stageIndex,project.stages.size) { mutableStateOf<Int?>(null) }
    var teamDialog by remember { mutableStateOf(false) }
    var stageDialog by remember { mutableStateOf(false) }
    var exportDialog by remember { mutableStateOf(false) }
    var rankDialog by remember { mutableStateOf(false) }
    var historyDialog by remember { mutableStateOf(false) }
    var actions by remember { mutableStateOf(false) }
    var fit by remember { mutableIntStateOf(0) }
    var pendingExport by remember { mutableStateOf<PngRequest?>(null) }
    var exportPodium by rememberSaveable { mutableStateOf(false) }
    var exportScale by remember { mutableIntStateOf(1) }
    var exportMargin by remember { mutableIntStateOf(36) }
    var exportTransparent by remember { mutableStateOf(false) }

    val savePng = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("image/png")) { uri ->
        val request = pendingExport
        pendingExport = null
        if (uri != null && request != null) model.fileWork("PNG сохранён") {
            requireNotNull(resolver.openOutputStream(uri)).use { out ->
                ScenePainter.writePng(request.project,request.slices,request.scale,request.margin,request.transparent,request.podium,out,font)
            }
        }
    }
    val saveProject = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri ->
        if (uri != null) model.fileWork("Проект экспортирован") {
            requireNotNull(resolver.openOutputStream(uri)).bufferedWriter().use { it.write(projectJson.encodeToString(project)) }
        }
    }
    val openProject = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) model.create("Проект импортирован") {
            requireNotNull(resolver.openInputStream(uri)).bufferedReader().use { projectJson.decodeFromString<Project>(it.readText()) }
        }
    }

    Surface(Modifier.fillMaxSize(), color = Color(DARK)) {
        BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding()) {
        val compact = maxHeight < 450.dp
        Column {
            Column(Modifier.fillMaxWidth().background(Color(PANEL)).padding(horizontal = 16.dp, vertical = if(compact) 2.dp else 12.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column {
                        Text("BRACKETLAB CS2", color = Color(AMBER), fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.sp)
                        Text("by gearbarium", color = Color(MUTED), fontSize = 10.sp)
                    }
                    Spacer(Modifier.weight(1f))
                    Box {
                        TextButton(onClick = { actions = true }) { Text("Проект") }
                        DropdownMenu(expanded = actions, onDismissRequest = { actions = false }) {
                            DropdownMenuItem(text = { Text("Новый турнир") }, onClick = {
                                actions = false
                                if (screen.ready) model.create("Создан турнир") { Project() }
                            })
                            DropdownMenuItem(text = { Text("Менеджер турниров") }, onClick = { actions = false; manager = true })
                            if (stage != null) {
                                DropdownMenuItem(text = { Text("Призовые места") }, onClick = { actions=false; podiumOpen=!podiumOpen })
                                DropdownMenuItem(text = { Text("Удалить текущий этап") }, onClick = { actions=false; deleteStage=true })
                            }
                            DropdownMenuItem(text = { Text("Импорт JSON") }, onClick = {
                                actions = false; openProject.launch(arrayOf("application/json", "text/plain"))
                            })
                            DropdownMenuItem(text = { Text("Экспорт JSON") }, onClick = {
                                actions = false; saveProject.launch("tournament.json")
                            })
                            DropdownMenuItem(text = { Text("Отменить действие") }, onClick = { actions = false; model.step(false) })
                            DropdownMenuItem(text = { Text("Повторить действие") }, onClick = { actions = false; model.step(true) })
                            DropdownMenuItem(text = { Text("История действий") }, onClick = { actions = false; historyDialog = true })
                            DropdownMenuItem(text = { Text(if (reduceMotion) "Включить движение" else "Уменьшить движение") },
                                onClick = { actions = false; model.reduceMotion(!reduceMotion) })
                        }
                    }
                }
                Text(project.name, fontSize = if(compact) 16.sp else 25.sp, fontWeight = FontWeight.SemiBold, maxLines = if(compact) 1 else 2)
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { teamDialog = true }, enabled = screen.ready) { Text("Команды · ${project.teams.size}") }
                    Button(onClick = { if (project.teams.size >= 2) stageDialog = true else teamDialog = true }, enabled = screen.ready) {
                        Text("Новый этап")
                    }
                }
                if (project.stages.isNotEmpty()) {
                    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        project.stages.forEachIndexed { index, item ->
                            FilterChip(selected = index == stageIndex, onClick = { selectedStage = index }, label = { Text(item.name) })
                        }
                    }
                }
            }
            if (stage == null) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("Соберите турнир", fontSize = 25.sp, fontWeight = FontWeight.SemiBold)
                        Spacer(Modifier.height(8.dp))
                        Text("Добавьте команды и создайте этап", color = Color(MUTED))
                    }
                }
            } else {
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(onClick = { fit++ }) { Text("Вписать") }
                    TextButton(onClick = { exportDialog = true }) { Text("Экспорт PNG") }
                    TextButton(onClick = { rankDialog = true }) { Text("Таблица") }
                    TextButton(onClick = { podiumOpen = !podiumOpen }) { Text("Призовые места") }
                    TextButton(onClick = { deleteStage = true }) { Text("Удалить этап") }
                    if (stage.format == Format.SWISS || stage.format == Format.ROUND_ROBIN) {
                        var open by remember { mutableStateOf(false) }
                        Box {
                            TextButton(onClick = { open = true }) { Text(roundFilter?.let { "Тур ${it + 1}" } ?: "Все туры") }
                            DropdownMenu(open, { open = false }) {
                                DropdownMenuItem(text = { Text("Все туры") }, onClick = { roundFilter = null; open = false })
                                stage.matches.map { it.round }.distinct().sorted().forEach { round ->
                                    DropdownMenuItem(text = { Text("Тур ${round + 1}") }, onClick = { roundFilter = round; open = false })
                                }
                            }
                        }
                    }
                    if (stage.format == Format.SWISS && !stage.swissFinished) TextButton(onClick = {
                        model.update("Тур добавлен") { old ->
                            old.replaceStage(stageIndex) { it.nextSwiss(it.matches.first().bo) }
                        }
                    }) { Text("Следующий тур") }
                    if (stage.format == Format.SWISS && !stage.swissFinished && runCatching { stage.qualifiers() }.isSuccess) {
                        TextButton(onClick = { model.update("Этап Swiss завершён") { old ->
                            old.replaceStage(stageIndex) { it.copy(swissFinished=true) }
                        } }) { Text("Завершить Swiss") }
                    }
                }
                BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
                    val wide=maxWidth >= 720.dp
                    Row(Modifier.fillMaxSize()) {
                        key(screen.tournamentId,stageIndex) {
                            BracketViewport(project,stage,fit,reduceMotion,selectedMatch,roundFilter,
                                Modifier.weight(1f).fillMaxHeight(),font,onSelect={ selectedMatch=it; editMatch=true })
                        }
                        if (wide && podiumOpen) Column(Modifier.width(245.dp).fillMaxHeight().background(Color(PANEL))
                            .verticalScroll(rememberScrollState()).padding(12.dp)) { PodiumContent(project,reduceMotion) }
                    }
                    if (!wide && podiumOpen) PodiumSheet(project,reduceMotion) { podiumOpen=false }
                }
            }
            if (screen.message.isNotBlank()) Text(screen.message, Modifier.fillMaxWidth().background(Color(PANEL)).padding(12.dp),
                fontSize = 13.sp, color = Color(MUTED))
        }
    }

    }
    if (manager) TournamentManager(screen,model) { manager=false }
    if (deleteStage && stage != null) {
        val dependency = remember(project,stageIndex) { runCatching { project.removeStage(stageIndex) }.exceptionOrNull()?.message }
        EditorDialog(onDismissRequest={deleteStage=false},title={Text("Удалить этап «${stage.name}»?")},
            text={Column {
                Text("Матчей: ${stage.matches.size} · Результатов: ${stage.matches.count { it.score != null }}")
                dependency?.let { Text(it,color=MaterialTheme.colorScheme.error) }
            }},confirmButton={
                Button(enabled=dependency==null,onClick={
                    model.update("Этап удалён") { it.removeStage(stageIndex) }
                    deleteStage=false
                }) { Text("Удалить этап") }
            },dismissButton={TextButton(onClick={deleteStage=false}) { Text("Отмена") }})
    }
    if (teamDialog) TeamsDialog(project, onDismiss = { teamDialog = false }) { name, teams ->
        model.update("Турнир и команды сохранены") { it.copy(name = name, teams = teams) }
        teamDialog = false
    }
    if (stageDialog) StageDialog(project, stageIndex, onDismiss = { stageDialog = false }) { result ->
        val warning = result.any { it.format == Format.ROUND_ROBIN && it.maxConsecutiveMatches() > 2 }
        model.update(if (warning) "В расписании есть серии из более чем двух встреч подряд" else "Этап создан") {
            it.copy(stages = it.stages + result)
        }
        selectedStage = project.stages.size
        stageDialog = false
    }
    if (rankDialog && stage != null) EditorDialog(
        onDismissRequest = { rankDialog = false }, title = { Text("Таблица этапа") },
        text = { Column(Modifier) {
            runCatching { stage.qualifiers().firstOrNull() }.getOrNull()?.let {
                Text("Лидер · ${project.teamName(it)}", fontWeight = FontWeight.SemiBold)
            }
            stage.standings().forEachIndexed { i, row ->
                Text("${i + 1}. ${project.teamName(row.first)}   ${row.second}–${row.third}", Modifier.padding(vertical = 6.dp))
            }
        } },
        confirmButton = { TextButton(onClick = { rankDialog = false }) { Text("Закрыть") } }
    )
    if (historyDialog) {
        val events by model.events.collectAsStateWithLifecycle(emptyList())
        EditorDialog(onDismissRequest = { historyDialog = false }, title = { Text("История действий") },
            text = { Column(Modifier) {
                val format = java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT)
                for (event in events) {
                    Text(event.action, fontWeight = FontWeight.SemiBold)
                    if (event.atMillis != 0L) Text(format.format(java.util.Date(event.atMillis)), color = Color(MUTED))
                    Spacer(Modifier.height(6.dp))
                }
            } }, confirmButton = { TextButton(onClick = { historyDialog = false }) { Text("Закрыть") } })
    }
    val selected = selectedMatch
    if (editMatch && selected != null && stage != null && selected in stage.matches.indices) {
        MatchDialog(project, stage, selected, onDismiss = { editMatch = false }) { replacement, message ->
            model.update(message) { old ->
                old.replaceStage(stageIndex) {
                    check(it == stage) { "Этап изменился. Откройте матч снова." }
                    replacement
                }
            }
            editMatch = false
        }
    }

    if (exportDialog && stage != null) {
        var scope by remember { mutableIntStateOf(0) }
        var round by remember { mutableIntStateOf(0) }
        val picked=selectedMatch?.takeIf { it in stage.matches.indices }
        val rounds=stage.matches.map { it.round }.distinct().sorted()
        val slice = if (scope == 3 && picked != null) ExportSlice.FromMatch(picked)
            else if (scope == 2) ExportSlice.Round(rounds[round]) else ExportSlice.All
        EditorDialog(onDismissRequest = { exportDialog = false }, title = { Text("Экспорт PNG") }, text = {
            Column {
                listOf("Текущий этап", "Весь турнир").forEachIndexed { i, name ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(selected = scope == i, onClick = { scope = i })
                        Text(name)
                    }
                }
                Row(verticalAlignment=Alignment.CenterVertically) {
                    RadioButton(selected=scope==3,onClick={scope=3},enabled=picked!=null)
                    Text("Раунд выбранного матча и все последующие")
                }
                if (picked==null) Text("Коснитесь матча на сетке, чтобы выбрать начальный раунд.",color=Color(MUTED))
                if (slice is ExportSlice.FromMatch) {
                    stage.included(slice).map { stage.matches[it] }.groupBy { it.lane }.forEach { (lane,matches) ->
                        Text("$lane · раунды ${matches.map { it.round+1 }.distinct().sorted().joinToString()}",color=Color(MUTED))
                    }
                }
                if (scope==1) Row(verticalAlignment=Alignment.CenterVertically) {
                    Checkbox(exportPodium,{exportPodium=it}); Text("Включить призовые места")
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    RadioButton(selected = scope == 2, onClick = { scope = 2; picked?.let { round=rounds.indexOf(stage.matches[it].round) } })
                    Text("Раунд ${rounds[round] + 1}")
                    TextButton(onClick = {
                        round = (round + 1) % rounds.size
                        scope = 2
                    }) { Text("Далее") }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(1,2).forEach { value ->
                        FilterChip(selected = exportScale == value, onClick = { exportScale = value }, label = { Text("${value}×") })
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(exportTransparent, { exportTransparent = it })
                    Text("Прозрачный фон")
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Поля")
                    TextButton(onClick = { exportMargin = when (exportMargin) { 24 -> 36; 36 -> 64; else -> 24 } }) {
                        Text("$exportMargin px")
                    }
                }
            }
        }, confirmButton = {
            Button(onClick = {
                val slices=if (scope==1) project.stages.indices.map { it to ExportSlice.All } else listOf(stageIndex to slice)
                pendingExport=PngRequest(project,slices,exportScale,exportMargin,exportTransparent,scope==1 && exportPodium)
                exportDialog = false
                savePng.launch("bracket.png")
            }) { Text("Сохранить") }
        })
    }
}

private fun Project.replaceStage(index: Int, transform: (Stage) -> Stage): Project =
    copy(stages = stages.mapIndexed { i, item -> if (i == index) transform(item) else item })

@Composable
private fun BracketViewport(project: Project, stage: Stage, fit: Int, reduceMotion: Boolean, selected: Int?,
    roundFilter: Int?, modifier: Modifier, font: android.graphics.Typeface, onSelect: (Int) -> Unit) {
    val round = roundFilter?.takeIf { selectedRound -> stage.matches.any { it.round == selectedRound } }
    val scene = remember(stage) { BracketScene(stage) }
    val logos = remember(project.teams.map { it.id to it.logoPng }) { ScenePainter.decodeLogos(project.teams) }
    val painter = remember(project.teams, scene, font, logos) { ScenePainter(project,scene,font,logos) }
    var dimensions by remember { mutableStateOf(IntSize.Zero) }
    var zoom by remember { mutableFloatStateOf(1f) }
    var pan by remember { mutableStateOf(Offset.Zero) }
    val active by rememberUpdatedState(onSelect)
    val density = LocalDensity.current.density
    var centered by remember { mutableStateOf(false) }
    var gestures by remember { mutableIntStateOf(0) }
    val pulse = remember { Animatable(0f) }
    val scores = stage.matches.map { it.score }
    var previousScores by remember { mutableStateOf(scores) }
    var affected by remember { mutableStateOf<Set<Int>>(emptySet()) }
    val gridAlpha by animateFloatAsState(if (selected == null) 1f else 0.55f,
        animationSpec = if (reduceMotion) snap() else tween(160), label = "Контраст поля")

    LaunchedEffect(scores, reduceMotion) {
        val changed = scores.indices.filter { scores[it] != null && scores[it] != previousScores.getOrNull(it) }
        previousScores = scores
        if (changed.isNotEmpty() && !reduceMotion) {
            affected = stage.matches.indices.filter { index ->
                index in changed || stage.matches[index].sources.any { source ->
                    (source.type == "winner" || source.type == "loser") && source.index() in changed
                }
            }.toSet()
            pulse.snapTo(1f)
            pulse.animateTo(0f, tween(460))
        } else {
            pulse.snapTo(0f)
        }
    }
    val structure = remember(stage.matches) { stage.matches.map { Triple(it.lane,it.round,it.sources) } }
    LaunchedEffect(structure, dimensions, fit, round, reduceMotion) {
        if (dimensions.width > 0 && dimensions.height > 0) {
            val bounds = if (round == null) scene.bounds
                else RectF().apply { scene.cards.filter { stage.matches[it.id].round == round }.forEach { union(it.rect) } }
            val fitZoom = minOf((dimensions.width - 40) / bounds.width(),
                (dimensions.height - 40) / bounds.height()).coerceIn(0.08f, 4f)
            val readableZoom = (minOf((dimensions.width - 32 * density) / CARD_WIDTH, density)).coerceAtLeast(0.08f)
            val openAtReadableZoom = fit == 0 && round == null && fitZoom < readableZoom
            val target = if (openAtReadableZoom) readableZoom else fitZoom
            val destination = if (openAtReadableZoom) Offset(16 * density, 22 * density)
                else Offset((dimensions.width - bounds.width() * target) / 2 - bounds.left * target,
                    (dimensions.height - bounds.height() * target) / 2 - bounds.top * target)
            if (centered && !reduceMotion) {
                val initialZoom = zoom
                val initialPan = pan
                val sequence = gestures
                animate(0f, 1f, animationSpec = tween(350)) { progress, _ ->
                    if (sequence == gestures) {
                        zoom = initialZoom + (target - initialZoom) * progress
                        pan = initialPan + (destination - initialPan) * progress
                    }
                }
            } else {
                zoom = target
                pan = destination
            }
            centered = true
        }
    }
    Canvas(modifier.fillMaxWidth().onSizeChanged { dimensions = it }
        .pointerInput(scene) {
            detectTransformGestures { center, delta, scale, _ ->
                gestures++
                val next = (zoom * scale).coerceIn(0.08f, 4f)
                pan = center + delta - (center - pan) * (next / zoom)
                zoom = next
            }
        }
        .pointerInput(scene, round) {
            fun select(point: Offset) {
                val local = (point - pan) / zoom
                scene.cards.lastOrNull { (round == null || stage.matches[it.id].round == round)
                    && it.rect.contains(local.x, local.y) }?.let { active(it.id) }
            }
            detectTapGestures(onTap = ::select, onLongPress = ::select)
        }) {
        val step = (28 * zoom).coerceAtLeast(16f)
        var x = ((pan.x % step) + step) % step
        while (x < size.width) {
            var y = ((pan.y % step) + step) % step
            while (y < size.height) {
                drawCircle(Color(DIVIDER).copy(alpha = gridAlpha), radius = 1f, center = Offset(x, y))
                y += step
            }
            x += step
        }
        val native = drawContext.canvas.nativeCanvas
        native.save()
        native.clipRect(0f, 0f, size.width, size.height)
        native.translate(pan.x, pan.y)
        native.scale(zoom, zoom)
        painter.paint(native, RectF(-pan.x / zoom, -pan.y / zoom, (size.width - pan.x) / zoom,
            (size.height - pan.y) / zoom), zoom / density, selected, round, affected, pulse.value)
        native.restore()
    }
}

@Composable
private fun TeamsDialog(project: Project, onDismiss: () -> Unit, save: (String, List<Team>) -> Unit) {
    var name by remember { mutableStateOf(project.name) }
    var teams by remember { mutableStateOf(project.teams.map(Team::withPlayerIds)) }
    var bulk by remember { mutableStateOf("") }
    var expanded by remember { mutableStateOf<String?>(null) }
    var logoTeam by remember { mutableStateOf<String?>(null) }
    var message by remember { mutableStateOf("") }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val logoPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        val id = logoTeam
        if (id != null && uri != null) scope.launch {
            try {
                val logo = withContext(Dispatchers.IO) { thumbnail(context, uri) }
                teams = teams.map { if (it.id == id) it.copy(logoPng = logo) else it }
            } catch (error: kotlinx.coroutines.CancellationException) {
                throw error
            } catch (error: Exception) {
                message = error.message ?: "Не удалось прочитать логотип"
            }
        }
    }
    val added = bulk.lineSequence().map(String::trim).filter(String::isNotEmpty).toList()
    EditorDialog(onDismissRequest = onDismiss, title = { Text("Турнир и команды") }, text = {
        Column(Modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(name, { name = it }, label = { Text("Название турнира") }, singleLine = true)
            for ((index, team) in teams.withIndex()) {
                fun change(updated: Team) {
                    teams = teams.mapIndexed { i, current -> if (i == index) updated else current }
                }
                OutlinedTextField(team.name, { text ->
                    change(team.copy(name = text))
                }, label = { Text("Команда ${index + 1}") }, singleLine = true)
                TextButton(onClick = { expanded = if (expanded == team.id) null else team.id }) { Text("Сведения о команде") }
                if (expanded == team.id) {
                    val data = remember(team.logoPng) { team.logoPng?.let { runCatching { Base64.decode(it, Base64.DEFAULT) }.getOrNull() } }
                    Text("ЛОГОТИП КОМАНДЫ",color=Color(MUTED))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(104.dp).background(Color(CARD),MaterialTheme.shapes.medium).padding(8.dp),
                            contentAlignment=Alignment.Center) {
                            if (data != null) AsyncImage(data, "Логотип команды", Modifier.fillMaxSize(), contentScale=ContentScale.Fit)
                            else Text("Без логотипа",color=Color(MUTED),fontSize=11.sp)
                        }
                        Column {
                            TextButton(onClick = { logoTeam = team.id; logoPicker.launch(arrayOf("image/png", "image/jpeg", "image/webp")) }) {
                                Text("Изменить")
                            }
                            if (data != null) TextButton(onClick = { change(team.copy(logoPng = null)) }) { Text("Удалить") }
                        }
                    }
                    OutlinedTextField(team.description, { change(team.copy(description = it)) }, label = { Text("Описание") })
                    for ((i,player) in team.players.withIndex()) key(player.id) {
                        var editing by rememberSaveable { mutableStateOf(false) }
                        Card(Modifier.fillMaxWidth()) {
                            Column(Modifier.padding(12.dp)) {
                                Text(player.name.ifBlank { "Игрок ${i+1}" },fontWeight=FontWeight.SemiBold)
                                if (player.steamId.isNotBlank()) Text("SteamID: ${player.steamId}",color=Color(MUTED))
                                Row {
                                    TextButton(onClick={editing=!editing}) { Text("Редактировать") }
                                    TextButton(onClick={ change(team.copy(players=team.players.filterNot { it.id==player.id })) }) { Text("Удалить") }
                                }
                                if (editing) {
                                    fun changePlayer(updated: Player) {
                                        change(team.copy(players=team.players.map { if (it.id==updated.id) updated else it }))
                                    }
                                    OutlinedTextField(player.name,{changePlayer(player.copy(name=it))},Modifier.fillMaxWidth(),label={Text("ФИО")})
                                    OutlinedTextField(player.steamId,{changePlayer(player.copy(steamId=it))},Modifier.fillMaxWidth(),label={Text("SteamID")})
                                    OutlinedTextField(player.note,{changePlayer(player.copy(note=it))},Modifier.fillMaxWidth(),label={Text("Примечание")})
                                }
                            }
                        }
                    }
                    TextButton(onClick={ change(team.copy(players=team.players+Player("","","",UUID.randomUUID().toString()))) }) { Text("+ Игрок") }
                }
            }
            OutlinedTextField(bulk, { bulk = it }, minLines = 3, label = { Text("Одна команда на строку") })
            Text("${teams.size + added.size} / 64", color = Color(MUTED))
            if (message.isNotEmpty()) Text(message, color = MaterialTheme.colorScheme.error)
        }
    }, confirmButton = {
        Button(enabled = name.isNotBlank() && teams.all { it.name.isNotBlank() } && teams.size + added.size <= 64, onClick = {
            save(name.trim(), teams + added.map { Team(UUID.randomUUID().toString(), it) })
        }) { Text("Сохранить") }
    }, dismissButton = { TextButton(onClick = onDismiss) { Text("Отмена") } })
}

@Composable
private fun StageDialog(project: Project, previous: Int, onDismiss: () -> Unit, save: (List<Stage>) -> Unit) {
    var name by remember { mutableStateOf("Этап ${project.stages.size + 1}") }
    var format by remember { mutableStateOf(Format.SINGLE) }
    var bo by remember { mutableIntStateOf(3) }
    var extra by remember { mutableStateOf(false) }
    var tieBreak by remember { mutableStateOf(TieBreak.SEED) }
    var ids by remember { mutableStateOf(project.teams.map(Team::id)) }
    var seed by remember { mutableStateOf<String?>(null) }
    var message by remember { mutableStateOf("") }
    var sourceStages by remember { mutableStateOf(if (project.stages.isEmpty()) emptyList() else listOf(previous)) }
    var qualification by remember { mutableStateOf<List<Qualification>>(emptyList()) }
    var showSources by remember { mutableStateOf(false) }
    var pass by remember { mutableStateOf("2") }
    var groupInput by remember { mutableStateOf("1") }
    var potInput by remember { mutableStateOf("") }
    var locked by remember { mutableStateOf<Set<String>>(emptySet()) }
    EditorDialog(onDismissRequest = onDismiss, title = { Text("Настройка этапа") }, text = {
        Column(Modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
            OutlinedTextField(name, { name = it }, label = { Text("Название этапа") }, singleLine = true)
            FlowFormat(format) { format = it; extra = false }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(1,3,5).forEach { value ->
                    FilterChip(selected = bo == value, onClick = { bo = value }, label = { Text("BO$value") })
                }
            }
            val option = when (format) {
                Format.SINGLE -> "Матч за третье место"
                Format.DOUBLE -> "Повторный гранд-финал"
                Format.ROUND_ROBIN -> "Два круга"
                else -> null
            }
            if (option != null) Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(extra, { extra = it }); Text(option)
            }
            if (format == Format.ROUND_ROBIN) {
                var open by remember { mutableStateOf(false) }
                Box {
                    OutlinedButton(onClick = { open = true }) { Text(tieBreak.label) }
                    DropdownMenu(open, { open = false }) {
                        TieBreak.entries.forEach { item ->
                            DropdownMenuItem(text = { Text(item.label) }, onClick = { tieBreak = item; open = false })
                        }
                    }
                }
            }
            if (project.stages.isNotEmpty()) {
                TextButton(onClick = { showSources = !showSources }) { Text("Квалификация из этапов") }
            }
            if (showSources) {
                project.stages.forEachIndexed { index, stage ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(index in sourceStages, { checked ->
                            sourceStages = if (checked) sourceStages + index else sourceStages - index
                        })
                        Text(stage.name)
                    }
                }
                OutlinedTextField(pass, { pass = it }, label = { Text("Число проходящих") }, singleLine = true)
                TextButton(onClick = {
                    val sources = sourceStages.map { Qualification(it, pass.toIntOrNull() ?: 2) }
                    runCatching { project.qualified(project.stages.size, sources) }.onSuccess {
                        ids = it; qualification = sources; seed = null
                    }.onFailure { message = it.message.orEmpty() }
                }) { Text("Взять лучших") }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(groupInput, { groupInput = it }, Modifier.weight(1f),
                    enabled = qualification.isEmpty(), label = { Text("Групп") }, singleLine = true)
                OutlinedTextField(potInput, { potInput = it }, Modifier.weight(1f),
                    label = { Text("Корзина") }, singleLine = true)
            }
            TextButton(onClick = {
                val value = kotlin.random.Random.nextLong()
                val groups = groupInput.toIntOrNull()?.coerceIn(1,32) ?: 1
                val size = potInput.toIntOrNull()?.coerceIn(1,64) ?: if (groups > 1) groups else ids.size.coerceAtLeast(1)
                ids = drawPots(ids, value.toULong(), size, locked)
                seed = value.toULong().toString()
            }) { Text("Перемешать") }
            Text("Посев", color = Color(MUTED))
            for ((index, id) in ids.withIndex()) Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(id in locked, { checked -> locked = if (checked) locked + id else locked - id })
                Text("${index + 1}. ${project.teamName(id)}", Modifier.weight(1f))
                TextButton(enabled = index > 0 && id !in locked && ids[index-1] !in locked, onClick = {
                    ids = ids.toMutableList().apply { add(index-1, removeAt(index)) }
                    seed = null
                }) { Text("↑") }
                TextButton(enabled = id !in locked, onClick = { ids = ids - id; seed = null; qualification = emptyList() }) { Text("Убрать") }
            }
            for (team in project.teams.filter { it.id !in ids }) {
                TextButton(onClick = { ids = ids + team.id; seed = null; qualification = emptyList() }) { Text("+ ${team.name}") }
            }
            if (message.isNotEmpty()) Text(message, color = MaterialTheme.colorScheme.error)
        }
    }, confirmButton = {
        Button(onClick = {
            runCatching {
                val groups = groupInput.toIntOrNull() ?: 1
                val generated = if (groups > 1 && qualification.isEmpty()) generateGroups(name.trim(), format, ids, groups, bo, extra, seed)
                else listOf(generateStage(name.trim(), format, ids, bo, extra, seed).copy(qualification = qualification))
                generated.map { if (format == Format.ROUND_ROBIN) it.copy(tieBreak = tieBreak) else it }
            }.onSuccess(save).onFailure { message = it.message.orEmpty() }
        }) { Text("Создать") }
    }, dismissButton = { TextButton(onClick = onDismiss) { Text("Отмена") } })
}

private fun thumbnail(context: android.content.Context, uri: Uri): String {
    val resolver = context.contentResolver
    val limits = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, limits) }
    require(limits.outWidth > 0 && limits.outHeight > 0) { "Неизвестный формат изображения" }
    val maximum = maxOf(limits.outWidth, limits.outHeight)
    var sample = 1
    while (maximum / sample > 192) sample *= 2
    val bitmap = requireNotNull(resolver.openInputStream(uri)?.use {
        BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample })
    })
    val scale = minOf(1f, 96f / maxOf(bitmap.width, bitmap.height))
    val fitted = Bitmap.createScaledBitmap(bitmap, (bitmap.width * scale).toInt().coerceAtLeast(1),
        (bitmap.height * scale).toInt().coerceAtLeast(1), true)
    return try {
        val bytes = ByteArrayOutputStream()
        check(fitted.compress(Bitmap.CompressFormat.PNG, 100, bytes))
        Base64.encodeToString(bytes.toByteArray(), Base64.NO_WRAP)
    } finally {
        if (fitted !== bitmap) fitted.recycle()
        bitmap.recycle()
    }
}

@Composable
private fun FlowFormat(selected: Format, choose: (Format) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        OutlinedButton(onClick = { open = true }) { Text(selected.title) }
        DropdownMenu(open, { open = false }) {
            Format.entries.forEach { item ->
                DropdownMenuItem(text = { Text(item.title) }, onClick = { choose(item); open = false })
            }
        }
    }
}

@Composable
private fun MatchDialog(project: Project, stage: Stage, index: Int, onDismiss: () -> Unit, save: (Stage, String) -> Unit) {
    val match = stage.matches[index]
    val state = stage.resolve()[index]
    var a by remember(index) { mutableStateOf((match.score?.series?.get(0) ?: 0).toString()) }
    var b by remember(index) { mutableStateOf((match.score?.series?.get(1) ?: 0).toString()) }
    var technical by remember(index) { mutableStateOf(match.score?.technical ?: false) }
    var note by remember(index) { mutableStateOf(match.score?.note.orEmpty()) }
    var maps by remember(index) { mutableStateOf(match.score?.maps.orEmpty()) }
    var message by remember(index) { mutableStateOf("") }
    var swap by remember(index) { mutableIntStateOf(-1) }
    fun submit(score: Score?) {
        runCatching { stage.changeScore(index, score) }
            .onSuccess { save(it, if (score == null) "Результат снят" else "Результат сохранён") }
            .onFailure { message = it.message.orEmpty() }
    }
    EditorDialog(onDismissRequest = onDismiss, title = { Text("Матч M%02d · BO%d".format(index+1, match.bo)) }, text = {
        Column(Modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(state.teams.joinToString("  /  ") { project.teamName(it) }, fontWeight = FontWeight.SemiBold)
            if (state.ready) {
                if (stage.format == Format.SWISS && match.round == stage.matches.last().round && match.score == null) {
                    Row {
                        repeat(2) { slot ->
                            TextButton(onClick = { swap = if (swap == slot) -1 else slot }) { Text("Заменить ${slot + 1}") }
                        }
                    }
                    if (swap >= 0) {
                        stage.matches.forEachIndexed { i, other ->
                            if (i != index && other.round == match.round && other.score == null && other.sources.all { it.type == "team" }) {
                                other.sources.forEach { source ->
                                    val id = requireNotNull(source.id).content
                                    TextButton(onClick = {
                                        runCatching { stage.swapSwiss(index, swap, id) }.onSuccess { save(it, "Пары обновлены") }
                                            .onFailure { message = it.message.orEmpty() }
                                    }) { Text(project.teamName(id)) }
                                }
                            }
                        }
                    }
                }
                if (technical || maps.isEmpty()) Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    OutlinedTextField(a, { a = it }, Modifier.weight(1f), label = { Text("Счёт 1") }, singleLine = true)
                    OutlinedTextField(b, { b = it }, Modifier.weight(1f), label = { Text("Счёт 2") }, singleLine = true)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(technical, { technical = it })
                    Text("Технический результат")
                }
                if (!technical) {
                    for ((i, map) in maps.withIndex()) {
                        fun replace(updated: MapScore) { maps = maps.mapIndexed { j, item -> if (j == i) updated else item } }
                        OutlinedTextField(map.name, { replace(map.copy(name = it)) }, label = { Text("Карта ${i+1}") })
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            for (slot in 0..1) {
                                OutlinedTextField(map.rounds[slot].toString(), { value ->
                                    val score = value.toIntOrNull()
                                    if (score != null && score in 0..255) {
                                        replace(map.copy(rounds = map.rounds.mapIndexed { j, old -> if (slot == j) score else old }))
                                    }
                                }, Modifier.weight(1f), singleLine = true)
                            }
                            TextButton(onClick = { maps = maps.filterIndexed { j, _ -> j != i } }) { Text("Убрать") }
                        }
                    }
                    if (maps.size < match.bo) TextButton(onClick = { maps = maps + MapScore("", listOf(0,0)) }) { Text("+ Карта") }
                }
                OutlinedTextField(note, { note = it }, label = { Text("Примечание") })
            }
            if (match.score != null) TextButton(onClick = { submit(null) }) { Text("Снять результат") }
            if (message.isNotEmpty()) Text(message, color = MaterialTheme.colorScheme.error)
        }
    }, confirmButton = {
        if (state.ready) Button(onClick = {
            val played = if (technical) emptyList() else maps
            val series = if (played.isEmpty()) listOf(a.toIntOrNull() ?: -1, b.toIntOrNull() ?: -1)
            else listOf(played.count { it.rounds[0] > it.rounds[1] }, played.count { it.rounds[1] > it.rounds[0] })
            submit(Score(series, technical, played, note))
        }) { Text("Сохранить") }
        else TextButton(onClick = onDismiss) { Text("Закрыть") }
    }, dismissButton = { if (state.ready) TextButton(onClick = onDismiss) { Text("Отмена") } })
}

private data class PngRequest(val project: Project, val slices: List<Pair<Int,ExportSlice>>, val scale: Int,
    val margin: Int, val transparent: Boolean, val podium: Boolean)

@Composable
private fun EditorDialog(
    onDismissRequest: () -> Unit, title: @Composable () -> Unit, text: @Composable () -> Unit,
    confirmButton: @Composable () -> Unit, dismissButton: (@Composable () -> Unit)? = null
) {
    Dialog(onDismissRequest,properties=DialogProperties(usePlatformDefaultWidth=false,decorFitsSystemWindows=false)) {
        BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(horizontal=12.dp),
            contentAlignment=Alignment.Center) {
            val compactIme = maxHeight < 170.dp
            Surface(Modifier.widthIn(max=600.dp).fillMaxWidth().padding(vertical=if(compactIme) 0.dp else 12.dp),
                shape=if(compactIme) MaterialTheme.shapes.medium else MaterialTheme.shapes.extraLarge,
                color=Color(PANEL),tonalElevation=3.dp) {
                Column(Modifier.padding(horizontal=16.dp,vertical=if(compactIme) 2.dp else 16.dp)) {
                    if (!compactIme) {
                        ProvideTextStyle(MaterialTheme.typography.titleLarge) { title() }
                        Spacer(Modifier.height(10.dp))
                    }
                    Column(Modifier.weight(1f,fill=false).fillMaxWidth().verticalScroll(rememberScrollState())
                        .padding(vertical=if(compactIme) 0.dp else 4.dp)) { text() }
                    if (!compactIme) {
                        Spacer(Modifier.height(8.dp))
                        Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.End,verticalAlignment=Alignment.CenterVertically) {
                            dismissButton?.invoke()
                            Spacer(Modifier.width(8.dp))
                            confirmButton()
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PodiumSheet(project: Project, reduceMotion: Boolean, dismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest=dismiss, containerColor=Color(PANEL),
        sheetState=rememberModalBottomSheetState(skipPartiallyExpanded=true)) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().verticalScroll(rememberScrollState()).padding(20.dp)) {
            PodiumContent(project,reduceMotion)
            TextButton(onClick=dismiss) { Text("Закрыть") }
        }
    }
}

@Composable
private fun PodiumContent(project: Project, reduceMotion: Boolean) {
    val places=remember(project.stages) { project.podium() }
    Text("Призовые места",style=MaterialTheme.typography.titleLarge)
    Spacer(Modifier.height(10.dp))
    places.forEachIndexed { i,id ->
        val accent=Color(PODIUM_COLORS[i])
        val reveal=animateFloatAsState(if (id == null) 0f else 1f,
            animationSpec=if (reduceMotion) snap() else tween(280),label="Призовое место").value
        OutlinedCard(Modifier.fillMaxWidth().padding(bottom=8.dp),
            colors=CardDefaults.outlinedCardColors(containerColor=androidx.compose.ui.graphics.lerp(Color(CARD),accent,0.08f*reveal)),
            border=androidx.compose.foundation.BorderStroke(1.dp,accent.copy(alpha=0.3f+0.45f*reveal))) {
            Row(Modifier.fillMaxWidth().padding(12.dp),verticalAlignment=Alignment.CenterVertically) {
                Text("${i+1}",color=accent,fontSize=24.sp,fontWeight=FontWeight.SemiBold,modifier=Modifier.width(32.dp))
                val logo=project.teams.firstOrNull { it.id==id }?.logoPng
                val data=remember(logo) { logo?.let { runCatching { Base64.decode(it,Base64.DEFAULT) }.getOrNull() } }
                if(data!=null) AsyncImage(data,"Логотип призёра",Modifier.size(36.dp).padding(end=6.dp),contentScale=ContentScale.Fit)
                Column {
                    Text("${i+1} место",color=accent)
                    Text(id?.let(project::teamName) ?: "Место пока не определено",color=if(id==null) Color(MUTED) else Color(TEXT))
                }
            }
        }
    }
}

@Composable
private fun TournamentManager(screen: ScreenState, model: TournamentViewModel, dismiss: () -> Unit) {
    var edit by remember { mutableStateOf<Pair<String,String>?>(null) }
    var delete by remember { mutableStateOf<Pair<String,String>?>(null) }
    var title by rememberSaveable { mutableStateOf("") }
    EditorDialog(onDismissRequest=dismiss,title={Text("Менеджер турниров")},text={
        Column(verticalArrangement=Arrangement.spacedBy(8.dp)) {
            if (screen.catalog.isEmpty()) Text("Сохранённых турниров пока нет")
            for (entry in screen.catalog) key(entry.id) {
                val project=remember(entry.payload) { runCatching { entry.project() }.getOrNull() }
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp)) {
                        Text(project?.name ?: "Проект не читается",style=MaterialTheme.typography.titleMedium)
                        if (project!=null) {
                            val status=when {
                                project.podium()[0]!=null && project.stages.all { runCatching { it.qualifiers() }.isSuccess && (it.format!=Format.SWISS || it.swissFinished) } -> "Завершён"
                                project.stages.any { stage -> stage.matches.any { it.score!=null } } -> "Идёт"
                                else -> "Подготовка"
                            }
                            Text("Команд: ${project.teams.size} · Этапов: ${project.stages.size} · $status",color=Color(MUTED))
                            val stamp=if(entry.updatedAt==0L) "" else " · "+java.text.DateFormat.getDateTimeInstance().format(java.util.Date(entry.updatedAt))
                            Text((project.stages.lastOrNull()?.format?.title ?: "Без этапов")+stamp,color=Color(MUTED))
                            Row {
                                TextButton(onClick={model.switch(entry.id);dismiss()}) { Text(if(screen.tournamentId==entry.id) "Текущий" else "Открыть") }
                                TextButton(onClick={edit=entry.id to project.name;title=project.name}) { Text("Переименовать") }
                            }
                            Row {
                                TextButton(onClick={model.create("Турнир продублирован"){project.copy(name=project.name+" — копия")};dismiss()}) { Text("Дублировать") }
                                TextButton(onClick={delete=entry.id to project.name}) { Text("Удалить") }
                            }
                        }
                    }
                }
            }
        }
    },confirmButton={Button(enabled=screen.ready,onClick={model.create("Создан турнир"){Project()};dismiss()}) { Text("Создать турнир") }},
        dismissButton={TextButton(onClick=dismiss) { Text("Закрыть") }})
    edit?.let { (id,_) ->
        EditorDialog(onDismissRequest={edit=null}, title={Text("Переименовать турнир")},
            text={OutlinedTextField(title,{title=it},Modifier.fillMaxWidth(),singleLine=true,label={Text("Название")})},
            confirmButton={Button(enabled=title.isNotBlank(),onClick={model.rename(id,title);edit=null}) { Text("Сохранить") }},
            dismissButton={TextButton(onClick={edit=null}) { Text("Отмена") }})
    }
    delete?.let { (id,name) ->
        EditorDialog(onDismissRequest={delete=null},title={Text("Удалить турнир «$name»?")},
            text={Text("Будут удалены команды, этапы, результаты и история турнира.")},
            confirmButton={Button(onClick={model.delete(id);delete=null}) { Text("Удалить турнир") }},
            dismissButton={TextButton(onClick={delete=null}) { Text("Отмена") }})
    }
}

