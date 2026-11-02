#!/bin/sh
# Prepare an eval of the agent skill and `vdt guide`: build the baseline and
# current vdt, and create one isolated Android project per agent and condition,
# each with a logging `vdt` shim and the prompt to give the agent.
#
# usage: tools/eval-guide/setup.sh [BASELINE_REF] [AGENT...]
#
#   BASELINE_REF  commit whose long skill and binary predate `vdt guide`
#                 (default a16b795)
#   AGENT         names for the runs (default: claude codex grok pi)
#
# Conditions:
#   long  baseline binary and the baseline's long SKILL.md
#   thin  current binary and the current short SKILL.md
#   none  current binary and no skill
#
# Runs go to $VDT_EVAL_DIR (default /tmp/vdt-eval). Give each agent the text
# of runs/<agent>-<condition>.prompt.txt, then grade with grade.py.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
E=${VDT_EVAL_DIR:-/tmp/vdt-eval}
baseline=${1:-a16b795}
[ $# -gt 0 ] && shift
agents=${*:-claude codex grok pi}

mkdir -p "$E/bin-old" "$E/bin-new" "$E/runs"
rm -rf "$E/src-old"
git -C "$repo" worktree add -q --detach "$E/src-old" "$baseline"
(cd "$E/src-old" && CARGO_TARGET_DIR="$E/target-old" cargo build --release -q)
cp "$E/target-old/release/vdt" "$E/bin-old/vdt"
git -C "$E/src-old" show HEAD:skills/vdt/SKILL.md > "$E/SKILL.long.md"
git -C "$repo" worktree remove --force "$E/src-old"
(cd "$repo" && cargo build --release -q)
cp "$repo/target/release/vdt" "$E/bin-new/vdt"

description=$(sed -n 's/^description: //p' "$repo/skills/vdt/SKILL.md")

for agent in $agents; do
    for condition in long thin none; do
        R=$E/runs/$agent-$condition
        rm -rf "$R"
        cp -R "$here/fixture" "$R"
        mkdir -p "$R/bin"
        bin=bin-new
        [ "$condition" = long ] && bin=bin-old
        cat > "$R/bin/vdt" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$R/.vdt-log"
exec $E/$bin/vdt "\$@"
EOF
        chmod +x "$R/bin/vdt"
        case $condition in
            long) mkdir -p "$R/.skills/vdt" && cp "$E/SKILL.long.md" "$R/.skills/vdt/SKILL.md" ;;
            thin) mkdir -p "$R/.skills/vdt" && cp "$repo/skills/vdt/SKILL.md" "$R/.skills/vdt/SKILL.md" ;;
        esac
        {
            echo "You are working in an Android project at $R. Start by running \`cd $R\`. Work only inside that directory: do not read, list, or search any file outside it (in particular not the directory you started in, and not other directories under $E), and do not use the network."
            echo
            echo "The \`vdt\` command-line tool is installed at $R/bin/vdt. Put it on your PATH first (\`export PATH=$R/bin:\$PATH\`) in every shell you open."
            if [ "$condition" != none ]; then
                echo
                echo "Your harness exposes this agent skill. Read its file when it is relevant to the work:"
                echo
                echo "- name: vdt"
                echo "  description: $description"
                echo "  file: $R/.skills/vdt/SKILL.md"
            fi
            sed "s|\$R|$R|g" "$here/prompt.md"
        } > "$E/runs/$agent-$condition.prompt.txt"
    done
done
echo "Prompts are in $E/runs/*.prompt.txt"
