package com.bracketlab

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

val projectJson = Json { encodeDefaults = true; explicitNulls = false }

@Serializable
data class Player(val name: String, @SerialName("steam_id") val steamId: String, val note: String, val id: String? = null)
@Serializable
data class Team(val id: String, val name: String, val description: String = "", val players: List<Player> = emptyList(),
    @SerialName("logo_png") val logoPng: String? = null)
@Serializable
enum class Format(val title: String) {
    @SerialName("single") SINGLE("Single Elimination"),
    @SerialName("double") DOUBLE("Double Elimination"),
    @SerialName("round_robin") ROUND_ROBIN("Round Robin"),
    @SerialName("swiss") SWISS("Swiss"),
    @SerialName("gsl") GSL("GSL")
}

@Serializable
data class Source(val type: String, val id: kotlinx.serialization.json.JsonPrimitive? = null) {
    fun index() = requireNotNull(id).content.toInt()
}

@Serializable
data class MapScore(val name: String, val rounds: List<Int>)
@Serializable
data class Score(val series: List<Int>, val technical: Boolean = false, val maps: List<MapScore> = emptyList(), val note: String = "")
@Serializable
data class Match(
    val round: Int, val lane: String, val sources: List<Source>, val bo: Int,
    val score: Score? = null, @SerialName("reset_of") val resetOf: Int? = null
)

@Serializable
data class Qualification(val stage: Int, val places: Int)

@Serializable
enum class TieBreak(val label: String) {
    @SerialName("seed") SEED("Посев"),
    @SerialName("maps") MAPS("Разница карт"),
    @SerialName("rounds") ROUNDS("Разница раундов")
}

@Serializable
data class Stage(
    val name: String, val format: Format, val entrants: List<String>,
    @SerialName("draw_seed") val drawSeed: String? = null, val matches: List<Match>,
    val qualification: List<Qualification> = emptyList(),
    @SerialName("tie_break") val tieBreak: TieBreak = TieBreak.SEED,
    @SerialName("swiss_finished") val swissFinished: Boolean = false
)

@Serializable
data class Project(val version: Int = 1, val name: String = "Новый турнир", val teams: List<Team> = emptyList(), val stages: List<Stage> = emptyList()) {
    fun teamName(id: String?): String = when (id) {
        null -> "Ожидается"
        BYE -> "BYE"
        else -> teams.find { it.id == id }?.name ?: "Команда удалена"
    }

    fun validate(): Project {
        val ids = teams.map(Team::id).toSet()
        require(version == 1 && ids.size == teams.size && ids.size <= 64) { "Некорректный проект" }
        ids.forEach(java.util.UUID::fromString)
        for (team in teams) {
            val players = team.players.mapNotNull(Player::id)
            require(players.toSet().size == players.size) { "Повторяющийся ID игрока в составе команды" }
            players.forEach(java.util.UUID::fromString)
        }
        require(teams.all { t -> t.logoPng?.let { it.length <= 100_000 && it.matches(Regex("[A-Za-z0-9+/=]+")) } ?: true })
        for ((stageIndex, stage) in stages.withIndex()) {
            require(stage.entrants.size in 2..64 && stage.entrants.distinct() == stage.entrants && stage.entrants.all(ids::contains))
            require(stage.matches.size in 1..4096)
            stage.matches.forEachIndexed { index, match ->
                require(match.round >= 0 && match.sources.size == 2 && match.bo in listOf(1, 3, 5))
                require(match.resetOf == null || match.resetOf in 0 until index)
                for (source in match.sources) {
                    when (source.type) {
                        "team" -> require(source.id?.content?.let(stage.entrants::contains) == true)
                        "winner", "loser" -> require(source.index() in 0 until index)
                        "bye" -> require(source.id == null)
                        else -> error("Неизвестный источник команды")
                    }
                }
            }
            val states = stage.resolve()
            stage.matches.forEachIndexed { index, match ->
                match.score?.let { validateScore(match.bo, it); require(states[index].ready) }
            }
            if (stage.qualification.isNotEmpty()) {
                require(qualified(stageIndex, stage.qualification).toSet() == stage.entrants.toSet()) {
                    "Редактирование изменило бы участников следующего этапа"
                }
            }
        }
        return this
    }

    fun qualified(before: Int, sources: List<Qualification>): List<String> {
        require(sources.map { it.stage }.distinct().size == sources.size) { "Повторяющийся этап" }
        return sources.flatMap { source ->
            require(source.stage in 0 until before && source.places > 0) { "Некорректная связь этапов" }
            val rank = stages[source.stage].qualifiers()
            require(rank.size >= source.places) { "Недостаточно участников для квалификации" }
            rank.take(source.places)
        }.distinct()
    }
}

private const val BYE = ""
data class Status(val teams: List<String?>, val result: List<String>? = null, val active: Boolean = true) {
    val ready get() = active && teams.all { !it.isNullOrEmpty() } && teams[0] != teams[1]
}

fun Stage.resolve(): List<Status> {
    val states = mutableListOf<Status>()
    for (game in matches) {
        val active = game.resetOf?.let {
            val previous = states[it]
            previous.result != null && !previous.teams[1].isNullOrEmpty() && previous.result[0] == previous.teams[1]
        } ?: true
        val teams = game.sources.map { source ->
            when (source.type) {
                "team" -> source.id?.content
                "bye" -> BYE
                "winner" -> states[source.index()].result?.get(0)
                "loser" -> states[source.index()].result?.get(1)
                else -> error("Неизвестный источник команды")
            }
        }
        val result = if (!active || teams.any { it == null }) null else {
            val (a, b) = teams.filterNotNull()
            when {
                a == BYE -> listOf(b, BYE)
                b == BYE -> listOf(a, BYE)
                game.score == null || game.bo / 2 + 1 !in game.score.series -> null
                game.score.series[0] > game.score.series[1] -> listOf(a, b)
                else -> listOf(b, a)
            }
        }
        states += Status(teams, result, active)
    }
    return states
}

private fun team(id: String) = Source("team", kotlinx.serialization.json.JsonPrimitive(id))
private fun winner(i: Int) = Source("winner", kotlinx.serialization.json.JsonPrimitive(i))
private fun loser(i: Int) = Source("loser", kotlinx.serialization.json.JsonPrimitive(i))
private val bye = Source("bye")

fun generateStage(name: String, format: Format, ids: List<String>, bo: Int, extra: Boolean, seed: String?): Stage {
    require(name.isNotBlank() && ids.size in 2..64 && ids.toSet().size == ids.size) { "Введите название и добавьте 2–64 команды" }
    require(bo in listOf(1, 3, 5))
    val games = mutableListOf<Match>()
    fun add(round: Int, lane: String, a: Source, b: Source, reset: Int? = null): Int {
        games += Match(round, lane, listOf(a, b), bo, resetOf = reset)
        return games.lastIndex
    }
    when (format) {
        Format.SINGLE, Format.DOUBLE -> {
            var order = listOf(0, 1)
            while (order.size < ids.size) {
                val last = order.size * 2 - 1
                order = order.flatMap { listOf(it, last - it) }
            }
            var sources = order.map { ids.getOrNull(it)?.let(::team) ?: bye }
            val upper = mutableListOf<List<Int>>()
            while (sources.size > 1) {
                val round = upper.size
                val idsInRound = sources.chunked(2).map { add(round, "Верхняя сетка", it[0], it[1]) }
                upper += idsInRound
                sources = idsInRound.map(::winner)
            }
            if (format == Format.SINGLE && extra && upper.size >= 2) {
                val (a, b) = upper[upper.lastIndex - 1]
                add(upper.lastIndex, "За третье место", loser(a), loser(b))
            }
            if (format == Format.DOUBLE) {
                var lower = upper.first().map(::loser)
                var round = 0
                for (drops in upper.drop(1)) {
                    lower = lower.chunked(2).map { winner(add(round, "Нижняя сетка", it[0], it[1])) }
                    round++
                    lower = lower.zip(drops.reversed()).map { (a, b) -> winner(add(round, "Нижняя сетка", a, loser(b))) }
                    round++
                }
                val finalRound = maxOf(round, upper.size)
                val grand = add(finalRound, "Гранд-финал", winner(upper.last().single()), lower.single())
                if (extra) add(finalRound + 1, "Гранд-финал", winner(grand), loser(grand), grand)
            }
        }
        Format.GSL -> {
            require(ids.size == 4) { "Для GSL нужны четыре команды" }
            val a = add(0, "Группа", team(ids[0]), team(ids[3]))
            val b = add(0, "Группа", team(ids[1]), team(ids[2]))
            val w = add(1, "Группа", winner(a), winner(b))
            val l = add(1, "Группа", loser(a), loser(b))
            add(2, "Группа", loser(w), winner(l))
        }
        Format.ROUND_ROBIN -> {
            val ring = ids.map(::team).toMutableList()
            if (ring.size % 2 != 0) ring += bye
            val rounds = ring.lastIndex
            repeat(rounds) { round ->
                repeat(ring.size / 2) { i ->
                    val a = ring[i]
                    val b = ring[ring.lastIndex - i]
                    if (a != bye && b != bye) add(round, "Туры", a, b)
                }
                ring.add(1, ring.removeAt(ring.lastIndex))
            }
            if (extra) for (game in games.toList()) add(game.round + rounds, "Туры", game.sources[1], game.sources[0])
        }
        Format.SWISS -> return Stage(name, format, ids, seed, games).nextSwiss(bo)
    }
    return Stage(name, format, ids, seed, games)
}

fun generateGroups(name: String, format: Format, ids: List<String>, groups: Int, bo: Int, extra: Boolean, seed: String?): List<Stage> {
    require(name.isNotBlank() && groups in 2..ids.size / 2) { "Нужно название и минимум две команды в каждой группе" }
    val allocation = List(groups) { mutableListOf<String>() }
    ids.chunked(groups).forEachIndexed { row, pot ->
        pot.forEachIndexed { column, id -> allocation[if (row % 2 == 0) column else groups - 1 - column].add(id) }
    }
    return allocation.mapIndexed { index, group -> generateStage("$name · Группа ${index+1}", format, group, bo, extra, seed) }
}

fun drawPots(ids: List<String>, seed: ULong, size: Int, locked: Set<String> = emptySet()): List<String> {
    var state = seed
    fun nextIndex(bound: ULong): Int {
        val threshold = (0uL - bound) % bound
        while (true) {
            state += 0x9e3779b97f4a7c15uL
            var word = state
            word = (word xor (word shr 30)) * 0xbf58476d1ce4e5b9uL
            word = (word xor (word shr 27)) * 0x94d049bb133111ebuL
            word = word xor (word shr 31)
            if (word >= threshold) return (word % bound).toInt()
        }
    }
    return ids.chunked(size.coerceAtLeast(1)).flatMap { pot ->
        val pool = pot.filterNot(locked::contains).toMutableList()
        for (i in pool.lastIndex downTo 1) {
            val j = nextIndex((i+1).toULong())
            val current = pool[i]
            pool[i] = pool[j]
            pool[j] = current
        }
        val shuffled = pool.iterator()
        pot.map { if (it in locked) it else shuffled.next() }
    }
}

fun Stage.swapSwiss(first: Int, slot: Int, replacement: String): Stage {
    val game = matches[first]
    require(format == Format.SWISS && game.round == matches.last().round && game.score == null &&
        game.sources.all { it.type == "team" } && slot in 0..1) {
        "Можно поменять команды только в несыгранных парах текущего тура"
    }
    val second = matches.indices.firstOrNull { idx ->
        val other = matches[idx]
        idx != first && other.round == game.round && other.score == null &&
            other.sources.all { it.type == "team" } && other.sources.any { it.id?.content == replacement }
    } ?: error("Команда недоступна для перестановки")
    val original = game.sources[slot]
    return copy(matches = matches.mapIndexed { index, match ->
        when (index) {
            first -> match.copy(sources = match.sources.mapIndexed { i, source -> if (i == slot) team(replacement) else source })
            second -> match.copy(sources = match.sources.map { if (it.id?.content == replacement) original else it })
            else -> match
        }
    })
}

fun Stage.standings(): List<Triple<String, Int, Int>> {
    val rows = entrants.associateWith { IntArray(4) }
    val states = resolve()
    for ((index, game) in matches.withIndex()) {
        val state = states[index]
        state.result?.let { result ->
            rows[result[0]]?.let { it[0]++ }
            rows[result[1]]?.let { it[1]++ }
        }
        val score = game.score
        if (score != null && !score.technical) {
            val maps = score.series[0] - score.series[1]
            val rounds = score.maps.sumOf { it.rounds[0] - it.rounds[1] }
            state.teams[0]?.let(rows::get)?.let { it[2] += maps; it[3] += rounds }
            state.teams[1]?.let(rows::get)?.let { it[2] -= maps; it[3] -= rounds }
        }
    }
    val tie = when (tieBreak) { TieBreak.SEED -> null; TieBreak.MAPS -> 2; TieBreak.ROUNDS -> 3 }
    return rows.entries.sortedWith(compareByDescending<Map.Entry<String, IntArray>> { it.value[0] }
        .thenBy { it.value[1] }.thenByDescending { if (tie == null) 0 else it.value[tie] })
        .map { Triple(it.key, it.value[0], it.value[1]) }
}

fun Stage.maxConsecutiveMatches(): Int {
    val streaks = mutableMapOf<String, Pair<Int, Int>>()
    var longest = 0
    matches.forEachIndexed { index, game ->
        game.sources.filter { it.type == "team" }.forEach { source ->
            val id = requireNotNull(source.id).content
            val past = streaks[id]
            val count = if (past?.first == index - 1) past.second + 1 else 1
            streaks[id] = index to count
            longest = maxOf(longest, count)
        }
    }
    return longest
}

fun Stage.qualifiers(): List<String> {
    val states = resolve()
    require(states.none { it.active && it.result == null }) { "Завершите все матчи этапа" }
    val rank = when (format) {
        Format.SINGLE, Format.DOUBLE -> {
            val final = matches.indices.last { states[it].active && matches[it].lane != "За третье место" }
            states[final].result.orEmpty() + matches.indices.reversed().flatMap {
                val result = states[it].result.orEmpty()
                if (matches[it].lane == "За третье место") result else result.drop(1)
            }
        }
        Format.GSL -> listOf(states[2].result!![0], states[4].result!![0], states[4].result!![1], states[3].result!![1])
        else -> standings().map { it.first }
    }
    return rank.filter(String::isNotEmpty).distinct()
}

fun Stage.nextSwiss(bo: Int): Stage {
    require(!swissFinished) { "Этап Swiss завершён" }
    val states = resolve()
    require(format == Format.SWISS && states.all { it.result != null }) { "Завершите предыдущий тур" }
    val round = (matches.lastOrNull()?.round ?: -1) + 1
    require(round < entrants.size) { "Достигнут предел туров" }
    val played = states.map(Status::teams).filter { it.all { id -> !id.isNullOrEmpty() } }
        .flatMap { listOf(it[0] to it[1], it[1] to it[0]) }.toSet()
    val ranked = standings().map { it.first }.toMutableList()
    val byeId = if (ranked.size % 2 == 1) {
        val index = ranked.indices.reversed().minBy { i -> states.count { it.teams == listOf(ranked[i], BYE) } }
        ranked.removeAt(index)
    } else null
    var budget = 20_000
    fun pair(remaining: List<String>): List<List<String>>? {
        if (remaining.isEmpty()) return emptyList()
        val a = remaining.first()
        for (b in remaining.drop(1)) {
            if (--budget < 0) return null
            if ((a to b) in played) continue
            val rest = pair(remaining.filter { it != a && it != b }) ?: continue
            return listOf(listOf(a, b)) + rest
        }
        return null
    }
    val next = (pair(ranked) ?: ranked.chunked(2)).map { ids -> Match(round, "Туры", ids.map(::team), bo) }
    val byeGame = byeId?.let { listOf(Match(round, "Туры", listOf(team(it), bye), bo)) }.orEmpty()
    return copy(matches = matches + next + byeGame)
}

private fun validateScore(bo: Int, result: Score) {
    val target = bo / 2 + 1
    val complete = result.series.count { it == target } == 1 && result.series.all { it in 0..target }
    val partial = result.maps.isNotEmpty() && !result.technical && result.series.all { it in 0 until target }
    require(result.series.size == 2 && (complete || partial)) {
        "Для BO$bo победителю нужно $target побед по картам"
    }
    require(!result.technical || result.maps.isEmpty()) { "Технический результат не содержит карт" }
    if (result.maps.isNotEmpty()) {
        val wins = mutableListOf(0, 0)
        for (map in result.maps) {
            require(map.name.isNotBlank() && map.rounds.size == 2 && map.rounds.all { it in 0..255 } && map.rounds[0] != map.rounds[1] && target !in wins) {
                "Проверьте название, счёт и порядок карт"
            }
            wins[if (map.rounds[0] > map.rounds[1]) 0 else 1]++
        }
        require(wins == result.series) { "Счёт серии не совпадает с картами" }
    }
}

fun Stage.changeScore(index: Int, score: Score?): Stage {
    val before = resolve()
    if (score != null) {
        require(before[index].ready) { "Матч ещё не готов" }
        validateScore(matches[index].bo, score)
    }
    val updated = copy(matches = matches.mapIndexed { i, game -> if (i == index) game.copy(score = score) else game })
    val after = updated.resolve()
    require(matches.indices.none { i -> i != index && matches[i].score != null &&
        (before[i].teams != after[i].teams || before[i].active != after[i].active) }) { "Сначала снимите результаты зависимых матчей" }
    require(format != Format.SWISS || matches[index].round == matches.last().round || before[index].result == after[index].result) {
        "Результат уже использован для следующего тура"
    }
    return updated
}

fun Project.removeStage(index: Int): Project {
    require(index in stages.indices)
    val dependent = stages.firstOrNull { stage -> stage.qualification.any { it.stage == index } }
    require(dependent == null) { "Этап «${dependent?.name}» зависит от этого этапа. Сначала удалите зависимый этап." }
    return copy(stages=stages.filterIndexed { i,_ -> i != index }.map { stage ->
        stage.copy(qualification=stage.qualification.map { if (it.stage > index) it.copy(stage=it.stage-1) else it })
    })
}

fun Project.podium(): List<String?> {
    val stage = stages.lastOrNull() ?: return List(3) { null }
    val sources = stages.flatMap { it.qualification }.map { it.stage }.toSet()
    if (stages.indices.count { it !in sources } > 1 && " · Группа " in stage.name) return List(3) { null }
    return stage.podium()
}

fun Stage.podium(): List<String?> {
    if (format == Format.SINGLE) {
        val states = resolve()
        val final = matches.indexOfLast { it.lane == "Верхняя сетка" }
        val third = matches.indexOfFirst { it.lane == "За третье место" }
        val result = states.getOrNull(final)?.result
        return listOf(result?.get(0), result?.get(1), states.getOrNull(third)?.result?.first()).map { it?.takeIf(String::isNotEmpty) }
    }
    if (format == Format.SWISS && !swissFinished) return List(3) { null }
    val ranked = runCatching { qualifiers() }.getOrElse { emptyList() }
    return List(3) { ranked.getOrNull(it) }
}

sealed interface ExportSlice {
    data object All : ExportSlice
    data class Round(val round: Int) : ExportSlice
    data class FromMatch(val index: Int) : ExportSlice
}

fun Stage.included(slice: ExportSlice): Set<Int> = when (slice) {
    ExportSlice.All -> matches.indices.toSet()
    is ExportSlice.Round -> matches.indices.filter { matches[it].round == slice.round }.toSet()
    is ExportSlice.FromMatch -> {
        val start = matches[slice.index]
        val starts = mutableMapOf(start.lane to start.round)
        var selected: Set<Int>
        do {
            selected = matches.indices.filter { i -> starts[matches[i].lane]?.let { matches[i].round >= it } == true }.toSet()
            var changed = false
            for (game in matches) {
                if (game.sources.any { it.type in listOf("winner","loser") && it.index() in selected }
                    && game.round < (starts[game.lane] ?: Int.MAX_VALUE)) {
                    starts[game.lane] = game.round
                    changed = true
                }
            }
        } while (changed)
        selected
    }
}

fun Match.mapRows(): List<Pair<String,String>> {
    val score = score ?: return emptyList()
    if (score.technical) return emptyList()
    if (score.maps.isEmpty() && bo > 1) return listOf("Детали карт не указаны" to "")
    return score.maps.map { it.name.uppercase() to "${it.rounds[0]} : ${it.rounds[1]}" }
}

fun Team.withPlayerIds(): Team = copy(players=players.map { player ->
    if (player.id == null) player.copy(id=java.util.UUID.randomUUID().toString()) else player
})

