package com.bracketlab

import org.junit.Assert.*
import org.junit.Test
import java.util.UUID

class TournamentTest {
    private fun teams(count: Int) = (1..count).map { UUID(0, it.toLong()).toString() }

    @Test fun exportFollowsDoubleEliminationBranches() {
        val stage = generateStage("Кубок",Format.DOUBLE,teams(16),5,true,null)
        val upper = stage.matches.indexOfFirst { it.lane=="Верхняя сетка" && it.round==1 }
        val included = stage.included(ExportSlice.FromMatch(upper))
        assertTrue(stage.matches.indices.filter { stage.matches[it].lane=="Верхняя сетка" && stage.matches[it].round>=1 }.all(included::contains))
        assertFalse(included.any { stage.matches[it].round==0 })
        assertTrue(included.any { stage.matches[it].lane=="Гранд-финал" })
        val lower = stage.matches.indexOfFirst { it.lane=="Нижняя сетка" && it.round==2 }
        assertFalse(stage.included(ExportSlice.FromMatch(lower)).any { stage.matches[it].lane=="Верхняя сетка" })
    }

    @Test fun podiumAndStageRemovalMatchWindows() {
        val ids=teams(8)
        val stage=complete(generateStage("Кубок",Format.SINGLE,ids,3,false,null))
        assertEquals(listOf(stage.qualifiers()[0],stage.qualifiers()[1],null),stage.podium())
        val bronze=complete(generateStage("Кубок",Format.SINGLE,ids,3,true,null))
        assertEquals(bronze.qualifiers().take(3),bronze.podium())
        val playoff=generateStage("Финал",Format.SINGLE,stage.qualifiers().take(2),3,false,null)
            .copy(qualification=listOf(Qualification(1,2)))
        val project=Project(teams=ids.map { Team(it,it) },stages=listOf(stage,stage,playoff))
        assertTrue(runCatching { project.removeStage(1) }.isFailure)
        val remaining=project.removeStage(0).validate()
        assertEquals(0,remaining.stages.last().qualification.single().stage)
    }

    @Test fun mapRowsDoNotInventResultsAndPartialScoreStaysOpen() {
        val stage=generateStage("Кубок",Format.SINGLE,teams(4),5,false,null)
        val partial=stage.changeScore(0,Score(listOf(1,0),maps=listOf(MapScore("Mirage",listOf(13,8)))))
        assertEquals(listOf("MIRAGE" to "13 : 8"),partial.matches[0].mapRows())
        assertNull(partial.resolve()[0].result)
        assertEquals(listOf("Детали карт не указаны" to ""),stage.changeScore(0,Score(listOf(3,0))).matches[0].mapRows())
        assertTrue(stage.changeScore(0,Score(listOf(3,0),technical=true)).matches[0].mapRows().isEmpty())
    }

    @Test fun legacyPlayersGainIdsAndKeepThemAcrossJson() {
        val team=Team(teams(2).first(),"Состав",players=listOf(Player("ФИО","123",""),Player("ФИО","456","")))
        val updated=team.withPlayerIds()
        assertNotEquals(updated.players[0].id,updated.players[1].id)
        val project=Project(teams=listOf(updated))
        assertEquals(project,projectJson.decodeFromString<Project>(projectJson.encodeToString(project)))
    }

    @Test fun sharedDrawAndLockedPots() {
        val ids = teams(8)
        val result = drawPots(ids, 42uL, ids.size)
        assertEquals(listOf(4, 2, 7, 3, 5, 1, 8, 6), result.map { ids.indexOf(it) + 1 })
        val locked = setOf(ids[1], ids[5])
        val reordered = drawPots(ids, 11uL, 4, locked)
        assertEquals(ids.take(4).toSet(), reordered.take(4).toSet())
        assertEquals(ids[1], reordered[1])
        assertEquals(ids[5], reordered[5])
    }

    @Test fun qualifiersFromFourGroups() {
        val ids = teams(16)
        val groups = generateGroups("Группа", Format.GSL, ids, 4, 3, false, null).map { complete(it) }
        val project = Project(teams = ids.map { Team(it, it) }, stages = groups)
        val sources = groups.indices.map { Qualification(it, 2) }
        val qualified = project.qualified(groups.size, sources)
        assertEquals(8, qualified.size)
        val playoff = generateStage("Плей-офф", Format.SINGLE, qualified, 3, false, null).copy(qualification = sources)
        project.copy(stages = groups + playoff).validate()
        assertTrue(runCatching {
            project.copy(stages = listOf(groups.first().changeScore(4, null)) + groups.drop(1) + playoff).validate()
        }.isFailure)
    }

    private fun complete(stage: Stage, secondWins: Set<Int> = emptySet()): Stage {
        var current = stage
        for (id in stage.matches.indices) {
            if (current.resolve()[id].ready) {
                val series = if (id in secondWins) listOf(0, 2) else listOf(2, 0)
                current = current.changeScore(id, Score(series))
            }
        }
        return current
    }

    @Test fun singleEliminationEveryTeamCount() {
        for (count in 2..64) {
            val ids = teams(count)
            val stage = complete(generateStage("Плей-офф", Format.SINGLE, ids, 3, false, null))
            assertEquals(count-1, stage.matches.count { it.score != null })
            assertEquals(ids.first(), stage.qualifiers().first())
        }
    }

    @Test fun doubleEliminationReset() {
        val stage = generateStage("Финал", Format.DOUBLE, teams(8), 3, true, null)
        val final = stage.matches.lastIndex - 1
        val played = complete(stage, setOf(final))
        assertNotNull(played.matches.last().score)
        assertEquals(played.resolve().last().result?.first(), played.qualifiers().first())
    }

    @Test fun roundRobinSevenIsBalanced() {
        val stage = generateStage("Лига", Format.ROUND_ROBIN, teams(7), 3, true, null)
        assertEquals(42, stage.matches.size)
        assertTrue(stage.maxConsecutiveMatches() <= 2)
        assertEquals(42, stage.matches.map(Match::sources).distinct().size)
        stage.matches.groupBy(Match::round).values.forEach { round ->
            assertEquals(6, round.flatMap(Match::sources).distinct().size)
        }
    }

    @Test fun swissAvoidsRepeats() {
        var stage = generateStage("Швейцарка", Format.SWISS, teams(7), 3, false, null)
        repeat(3) { stage = complete(stage).nextSwiss(3) }
        val real = stage.matches.filter { it.sources.none { source -> source.type == "bye" } }
        assertEquals(real.size, real.map { it.sources.toSet() }.distinct().size)
    }

    @Test fun refusesToChangePlayedParticipants() {
        val stage = complete(generateStage("Группа", Format.GSL, teams(4), 3, false, null))
        assertEquals(4, stage.qualifiers().size)
        assertTrue(runCatching { stage.changeScore(0, Score(listOf(0, 2))) }.isFailure)
    }

    @Test fun loadsRustProjectAndRoundTripsBye() {
        val a = teams(3)
        val input = """
            {"version":1,"name":"Кубок","teams":[
                {"id":"${a[0]}","name":"Один","description":"","players":[]},
                {"id":"${a[1]}","name":"Два","description":"","players":[]},
                {"id":"${a[2]}","name":"Три","description":"","players":[]}],
             "stages":[{"name":"Кубок","format":"single","entrants":["${a[0]}","${a[1]}","${a[2]}"],"draw_seed":null,"matches":[
               {"round":0,"lane":"Верхняя сетка","sources":[{"type":"team","id":"${a[0]}"},{"type":"bye"}],"bo":3,"score":null,"reset_of":null}
             ]}]}
        """.trimIndent()
        val project = projectJson.decodeFromString<Project>(input).validate()
        assertEquals(a[0], project.stages[0].resolve()[0].result?.first())
        assertEquals(project, projectJson.decodeFromString<Project>(projectJson.encodeToString(project)))
    }
}
