package com.bracketlab

import android.app.Application
import android.content.Context
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.room.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

private val Context.preferences by preferencesDataStore("settings")
private val reduceMotionKey = booleanPreferencesKey("reduce_motion")

@Entity
data class Revision(@PrimaryKey(autoGenerate = true) val id: Long = 0, val action: String, val payload: String,
    @ColumnInfo(defaultValue = "'legacy'") val tournamentId: String)

@Entity
data class Cursor(@PrimaryKey val singleton: Int = 1, val revision: Long)

@Entity
data class Audit(@PrimaryKey(autoGenerate = true) val id: Long = 0, val action: String, val atMillis: Long,
    @ColumnInfo(defaultValue = "'legacy'") val tournamentId: String)

@Entity
data class Tournament(@PrimaryKey val id: String, val revision: Long, val updatedAt: Long)
data class CatalogEntry(val id: String, val payload: String, val updatedAt: Long) {
    fun project() = projectJson.decodeFromString<Project>(payload).validate()
}

@Dao
interface History {
    @Query("SELECT payload FROM Revision JOIN Cursor ON Revision.id = Cursor.revision") suspend fun current(): String?
    @Query("SELECT tournamentId FROM Revision JOIN Cursor ON Revision.id = Cursor.revision") suspend fun active(): String?
    @Query("SELECT id FROM Revision WHERE tournamentId=:id AND id < :revision ORDER BY id DESC LIMIT 1")
    suspend fun previous(id: String, revision: Long): Long?
    @Query("SELECT id FROM Revision WHERE tournamentId=:id AND id > :revision ORDER BY id LIMIT 1")
    suspend fun next(id: String, revision: Long): Long?
    @Query("DELETE FROM Revision WHERE tournamentId=:id AND Revision.id > :revision")
    suspend fun discardAfter(id: String, revision: Long)
    @Query("SELECT Tournament.id, payload, updatedAt FROM Tournament JOIN Revision ON Revision.id=Tournament.revision ORDER BY updatedAt DESC,Tournament.id")
    suspend fun catalog(): List<CatalogEntry>
    @Query("SELECT * FROM Tournament WHERE id=:id") suspend fun tournament(id: String): Tournament?
    @Query("DELETE FROM Tournament WHERE id=:id") suspend fun remove(id: String)
    @Query("DELETE FROM Revision WHERE tournamentId=:id") suspend fun removeHistory(id: String)
    @Query("DELETE FROM Audit WHERE tournamentId=:id") suspend fun removeAudit(id: String)
    @Query("DELETE FROM Cursor") suspend fun clearCursor()
    @Upsert suspend fun save(entry: Tournament)
    @Insert suspend fun append(entry: Revision): Long
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun select(cursor: Cursor)
    @Insert suspend fun audit(event: Audit)
    @Query("SELECT * FROM Audit WHERE tournamentId=:id ORDER BY id DESC LIMIT 200") fun events(id: String?): Flow<List<Audit>>
}

@Database(entities = [Revision::class, Cursor::class, Audit::class,Tournament::class], version = 3, exportSchema = false)
abstract class TournamentDatabase : RoomDatabase() {
    abstract fun history(): History
}

data class ScreenState(val project: Project = Project(), val ready: Boolean = false, val message: String = "",
    val tournamentId: String? = null, val catalog: List<CatalogEntry> = emptyList())

class TournamentViewModel(application: Application) : AndroidViewModel(application) {
    private val db = Room.databaseBuilder(application, TournamentDatabase::class.java, "bracketlab.sqlite3")
        .addMigrations(object : androidx.room.migration.Migration(1, 2) {
            override fun migrate(db: androidx.sqlite.db.SupportSQLiteDatabase) {
                db.execSQL("CREATE TABLE IF NOT EXISTS Audit (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, action TEXT NOT NULL, atMillis INTEGER NOT NULL)")
                db.execSQL("INSERT INTO Audit (action, atMillis) SELECT action, 0 FROM Revision ORDER BY id")
            }
        }, object : androidx.room.migration.Migration(2, 3) {
            override fun migrate(db: androidx.sqlite.db.SupportSQLiteDatabase) {
                db.query("SELECT payload FROM Revision JOIN Cursor ON Revision.id=Cursor.revision").use { cursor ->
                    if (cursor.moveToFirst()) {
                        val backup = android.util.AtomicFile(application.filesDir.resolve("bracketlab-v2-backup.json"))
                        val stream = backup.startWrite()
                        try {
                            stream.write(cursor.getString(0).toByteArray(Charsets.UTF_8))
                            backup.finishWrite(stream)
                        } catch (error: Exception) {
                            backup.failWrite(stream)
                            throw error
                        }
                    }
                }
                db.execSQL("ALTER TABLE Revision ADD COLUMN tournamentId TEXT NOT NULL DEFAULT 'legacy'")
                db.execSQL("ALTER TABLE Audit ADD COLUMN tournamentId TEXT NOT NULL DEFAULT 'legacy'")
                db.execSQL("CREATE TABLE Tournament (id TEXT NOT NULL PRIMARY KEY, revision INTEGER NOT NULL, updatedAt INTEGER NOT NULL)")
                db.execSQL("INSERT INTO Tournament SELECT 'legacy',revision,(SELECT COALESCE(MAX(atMillis),0) FROM Audit) FROM Cursor")
            }
        }).build()
    private val preferences = application.preferences
    private val mutex = Mutex()
    private val mutableState = MutableStateFlow(ScreenState())
    val state = mutableState.asStateFlow()
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val events = state.map { it.tournamentId }.distinctUntilChanged().flatMapLatest { db.history().events(it) }
        .catch { emit(emptyList()) }
    val reduceMotion = preferences.data.map { it[reduceMotionKey] ?: false }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), false)

    init { operation { publish("") } }

    private suspend fun publish(message: String) {
        mutableState.value = db.withTransaction {
            val history = db.history()
            val project = history.current()?.let { projectJson.decodeFromString<Project>(it).validate() } ?: Project()
            ScreenState(project,true,message,history.active(),history.catalog())
        }
    }

    private suspend fun record(id: String, project: Project, action: String, activate: Boolean) {
        val history = db.history()
        history.tournament(id)?.let { history.discardAfter(id,it.revision) }
        val revision = history.append(Revision(action=action,payload=projectJson.encodeToString(project),tournamentId=id))
        val time = System.currentTimeMillis()
        history.save(Tournament(id,revision,time))
        if (activate) history.select(Cursor(revision=revision))
        history.audit(Audit(action=action,atMillis=time,tournamentId=id))
    }

    fun create(action: String, make: () -> Project) = operation {
        check(state.value.ready)
        val project = make().validate()
        db.withTransaction { record(java.util.UUID.randomUUID().toString(),project,action,true) }
        publish(action)
    }

    fun switch(id: String) = operation {
        db.withTransaction { db.history().select(Cursor(revision=requireNotNull(db.history().tournament(id)).revision)) }
        publish("Турнир открыт")
    }

    fun rename(id: String, name: String) = operation {
        require(name.isNotBlank())
        db.withTransaction {
            val entry = db.history().catalog().first { it.id == id }
            record(id,entry.project().copy(name=name.trim()),"Турнир переименован",db.history().active()==id)
        }
        publish("Турнир переименован")
    }

    fun delete(id: String) = operation {
        db.withTransaction {
            val history = db.history()
            val active = history.active()
            history.remove(id)
            if (active == id) {
                val next = history.catalog().firstOrNull()
                if (next == null) history.clearCursor()
                else history.select(Cursor(revision=requireNotNull(history.tournament(next.id)).revision))
            }
            history.removeHistory(id)
            history.removeAudit(id)
        }
        publish("Турнир удалён")
    }

    fun reduceMotion(enabled: Boolean) = operation { preferences.edit { it[reduceMotionKey] = enabled } }

    fun update(action: String, transform: (Project) -> Project) {
        val expected = state.value.tournamentId
        operation {
            check(state.value.ready && state.value.tournamentId == expected) { "Турнир сменился" }
            val project = transform(state.value.project).validate()
            db.withTransaction {
                val id = expected ?: java.util.UUID.randomUUID().toString()
                if (expected == null) record(id,Project(),"Новый турнир",true)
                record(id,project,action,true)
            }
            publish(action)
        }
    }

    fun step(redo: Boolean) = operation {
        val action = if (redo) "Повтор действия" else "Отмена действия"
        db.withTransaction {
            val history = db.history()
            val item = history.active()?.let { history.tournament(it) } ?: error("Нет действий")
            val revision = (if (redo) history.next(item.id,item.revision) else history.previous(item.id,item.revision))
                ?: error("Нет действий")
            val time = System.currentTimeMillis()
            history.save(item.copy(revision=revision,updatedAt=time))
            history.select(Cursor(revision=revision))
            history.audit(Audit(action=action,atMillis=time,tournamentId=item.id))
        }
        publish(action)
    }

    fun fileWork(action: String, block: suspend () -> Unit) = operation {
        block()
        mutableState.update { it.copy(message=action) }
    }

    private fun operation(block: suspend () -> Unit) {
        viewModelScope.launch {
            mutex.withLock {
                try { withContext(Dispatchers.IO) { block() } }
                catch (error: kotlinx.coroutines.CancellationException) { throw error }
                catch (error: Exception) { mutableState.update { it.copy(message=error.message ?: "Операция не выполнена") } }
            }
        }
    }
}
