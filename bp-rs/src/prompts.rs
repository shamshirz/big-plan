//! Embedded prompt slices for agent sessions.

pub const UNIVERSAL_GUIDANCE: &str = "\
You are running inside **big-plan** (`bp`), a project-local task loop.\n\
Each task gets a fresh context window — stay focused on the current task only.\n\
\n\
**Every session:**\n\
- Read task text: `bp read current` (or `bp read plan` for the goal plan)\n\
- Finish with: `bp complete --notes \"what changed + how you verified\"`\n\
- Inspect queue: `bp status` · retry: `bp reset <id>`\n\
\n\
Full skill reference: `.loop/SKILL.md`\n\
";

pub const PLAN_DECOMPOSITION_GUIDANCE: &str = "\
## Your job\n\
\n\
Decompose the plan below into a **task queue** for `bp`. Each task should benefit \
from its own fresh agent context window.\n\
\n\
## How to create good tasks\n\
\n\
- **One major concern per task** — independently reviewable and completable.\n\
- Split design from implementation, schema from integration, API from persistence.\n\
- Titles are imperative and concrete (`Add goals table migration`, not `Database work`).\n\
- Do **not** edit `.loop/` files or SQLite directly.\n\
\n\
## Commands for this planning session\n\
\n\
```bash\n\
bp add \"<title>\"          # create each executable task (repeat)\n\
bp status                   # verify the queue\n\
bp complete --notes \"...\"  # when decomposition is done\n\
```\n\
\n\
Read `.loop/SKILL.md` if you need the full workflow.\n\
";
