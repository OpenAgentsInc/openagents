#!/bin/bash
#
# Parallel Autopilot - Run multiple autopilot agents in isolated containers
#
# Usage:
#   parallel-autopilot.sh start [N]  - Start N agents (default: 3)
#   parallel-autopilot.sh stop       - Stop all agents
#   parallel-autopilot.sh status     - Show running agents and issues
#   parallel-autopilot.sh logs [N]   - Tail logs (all or specific agent)
#   parallel-autopilot.sh cleanup    - Remove worktrees and branches
#
# Environment variables:
#   AGENT_MEMORY  - Memory limit per agent (default: 12G on Linux, 3G on macOS)
#   AGENT_CPUS    - CPU limit per agent (default: 4 on Linux, 2 on macOS)
#

set -e

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$PROJECT_ROOT/docker/autopilot/docker-compose.yml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Detect platform and set defaults
detect_platform() {
    if [[ "$(uname)" == "Darwin" ]]; then
        PLATFORM="macos"
        DEFAULT_MEMORY="3G"
        DEFAULT_CPUS="2"
        MAX_AGENTS=5
    else
        PLATFORM="linux"
        DEFAULT_MEMORY="12G"
        DEFAULT_CPUS="4"
        MAX_AGENTS=10
    fi

    export AGENT_MEMORY="${AGENT_MEMORY:-$DEFAULT_MEMORY}"
    export AGENT_CPUS="${AGENT_CPUS:-$DEFAULT_CPUS}"
    export HOST_UID="$(id -u)"
}

# Create git worktrees for N agents
create_worktrees() {
    local count=$1
    echo -e "${BLUE}Creating $count worktrees...${NC}"

    cd "$PROJECT_ROOT"
    mkdir -p .worktrees

    for i in $(seq -w 1 "$count"); do
        local worktree_path=".worktrees/agent-$i"
        local branch_name="agent/$i"

        if [ -d "$worktree_path" ]; then
            echo -e "  ${YELLOW}Worktree $worktree_path already exists${NC}"
        else
            echo -e "  ${GREEN}Creating $worktree_path -> $branch_name${NC}"
            git worktree add "$worktree_path" -b "$branch_name" main 2>/dev/null || \
            git worktree add "$worktree_path" "$branch_name" 2>/dev/null || \
            echo -e "  ${RED}Failed to create worktree (branch may exist)${NC}"
        fi
    done
}

# Start N agents
cmd_start() {
    local count="${1:-3}"
    detect_platform

    echo -e "${BLUE}Platform: $PLATFORM${NC}"
    echo -e "${BLUE}Memory per agent: $AGENT_MEMORY${NC}"
    echo -e "${BLUE}CPUs per agent: $AGENT_CPUS${NC}"

    if [ "$count" -gt "$MAX_AGENTS" ]; then
        echo -e "${YELLOW}Warning: Requested $count agents, but max for $PLATFORM is $MAX_AGENTS${NC}"
        count=$MAX_AGENTS
    fi

    # Create worktrees
    create_worktrees "$count"

    # Ensure autopilot.db exists
    if [ ! -f "$PROJECT_ROOT/autopilot.db" ]; then
        echo -e "${YELLOW}Warning: autopilot.db not found, issues may not be available${NC}"
    fi

    # Build and start containers
    echo -e "${BLUE}Starting $count agents...${NC}"
    cd "$PROJECT_ROOT"

    # Determine which profile to use
    local profiles=""
    if [ "$count" -gt 3 ]; then
        profiles="--profile extended"
    fi
    if [ "$count" -gt 5 ] && [ "$PLATFORM" == "linux" ]; then
        profiles="$profiles --profile linux-full"
    fi

    # Start specific services based on count
    local services=""
    for i in $(seq -w 1 "$count"); do
        services="$services agent-$i"
    done

    docker-compose -f "$COMPOSE_FILE" $profiles up -d $services

    echo -e "${GREEN}Started $count agents${NC}"
    cmd_status
}

# Stop all agents
cmd_stop() {
    echo -e "${BLUE}Stopping all agents...${NC}"
    cd "$PROJECT_ROOT"
    docker-compose -f "$COMPOSE_FILE" --profile extended --profile linux-full down
    echo -e "${GREEN}All agents stopped${NC}"
}

# Show status
cmd_status() {
    echo -e "${BLUE}=== Running Containers ===${NC}"
    docker-compose -f "$COMPOSE_FILE" --profile extended --profile linux-full ps 2>/dev/null || true

    echo ""
    echo -e "${BLUE}=== Issue Status ===${NC}"
    if [ -f "$PROJECT_ROOT/autopilot.db" ]; then
        echo "Open issues:"
        sqlite3 "$PROJECT_ROOT/autopilot.db" \
            "SELECT printf('#%-4d [%-6s] %s', number, priority, substr(title, 1, 50)) FROM issues WHERE status='open' AND is_blocked=0 ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END, number LIMIT 10" \
            2>/dev/null || echo "  (no issues or database error)"

        echo ""
        echo "In progress:"
        sqlite3 "$PROJECT_ROOT/autopilot.db" \
            "SELECT printf('#%-4d [%-10s] %s', number, claimed_by, substr(title, 1, 40)) FROM issues WHERE status='in_progress' ORDER BY number" \
            2>/dev/null || echo "  (none)"
    else
        echo -e "${YELLOW}autopilot.db not found${NC}"
    fi
}

# Show issue queue
cmd_queue() {
    if [ ! -f "$PROJECT_ROOT/autopilot.db" ]; then
        echo -e "${YELLOW}autopilot.db not found${NC}"
        return 1
    fi

    echo -e "${BLUE}=== Open Issues Queue ===${NC}"
    echo ""

    # Count total open issues
    local total_open=$(sqlite3 "$PROJECT_ROOT/autopilot.db" \
        "SELECT COUNT(*) FROM issues WHERE status='open' AND is_blocked=0" 2>/dev/null)

    echo -e "${GREEN}Total open issues: $total_open${NC}"
    echo ""

    # Show by priority
    for priority in urgent high medium low; do
        local count=$(sqlite3 "$PROJECT_ROOT/autopilot.db" \
            "SELECT COUNT(*) FROM issues WHERE status='open' AND is_blocked=0 AND priority='$priority'" 2>/dev/null)

        if [ "$count" -gt 0 ]; then
            echo -e "${YELLOW}$priority priority ($count issues):${NC}"
            sqlite3 "$PROJECT_ROOT/autopilot.db" \
                "SELECT printf('  #%-4d %s', number, title) FROM issues WHERE status='open' AND is_blocked=0 AND priority='$priority' ORDER BY number LIMIT 10" \
                2>/dev/null
            echo ""
        fi
    done

    # Show in-progress
    local in_progress=$(sqlite3 "$PROJECT_ROOT/autopilot.db" \
        "SELECT COUNT(*) FROM issues WHERE status='in_progress'" 2>/dev/null)

    if [ "$in_progress" -gt 0 ]; then
        echo -e "${BLUE}In progress ($in_progress issues):${NC}"
        sqlite3 "$PROJECT_ROOT/autopilot.db" \
            "SELECT printf('  #%-4d [%-10s] %s', number, claimed_by, title) FROM issues WHERE status='in_progress' ORDER BY number" \
            2>/dev/null
    fi
}

# Tail logs
cmd_logs() {
    local agent_id="$1"
    cd "$PROJECT_ROOT"

    if [ -n "$agent_id" ]; then
        # Pad agent ID with leading zeros
        agent_id=$(printf "%03d" "$agent_id")
        echo -e "${BLUE}Tailing logs for agent-$agent_id...${NC}"
        docker logs -f "autopilot-$agent_id" 2>&1
    else
        echo -e "${BLUE}Tailing all agent logs...${NC}"
        docker-compose -f "$COMPOSE_FILE" --profile extended --profile linux-full logs -f
    fi
}

# Cleanup worktrees and branches
cmd_cleanup() {
    echo -e "${BLUE}Cleaning up worktrees and branches...${NC}"
    cd "$PROJECT_ROOT"

    # Stop containers first
    cmd_stop 2>/dev/null || true

    # Remove worktrees
    echo "Removing worktrees..."
    git worktree list | grep '.worktrees' | awk '{print $1}' | while read -r worktree; do
        echo -e "  ${YELLOW}Removing $worktree${NC}"
        git worktree remove --force "$worktree" 2>/dev/null || true
    done

    # Prune worktree references
    git worktree prune

    # Remove agent branches
    echo "Removing agent branches..."
    git branch | grep 'agent/' | while read -r branch; do
        branch=$(echo "$branch" | tr -d ' *')
        echo -e "  ${YELLOW}Deleting branch $branch${NC}"
        git branch -D "$branch" 2>/dev/null || true
    done

    # Remove .worktrees directory if empty
    rmdir .worktrees 2>/dev/null || true

    echo -e "${GREEN}Cleanup complete${NC}"
}

# Show help
cmd_help() {
    echo "Parallel Autopilot - Run multiple autopilot agents in isolated containers"
    echo ""
    echo "Usage: $0 <command> [args]"
    echo ""
    echo "Commands:"
    echo "  start [N]    Start N agents (default: 3, max: 10 on Linux, 5 on macOS)"
    echo "  stop         Stop all running agents"
    echo "  status       Show running agents and issue queue"
    echo "  queue        Show detailed issue queue by priority"
    echo "  logs [N]     Tail logs for agent N (or all if N not specified)"
    echo "  cleanup      Remove all worktrees and agent branches"
    echo "  help         Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  AGENT_MEMORY  Memory per agent (default: 12G on Linux, 3G on macOS)"
    echo "  AGENT_CPUS    CPUs per agent (default: 4 on Linux, 2 on macOS)"
    echo ""
    echo "Examples:"
    echo "  $0 start 5           # Start 5 agents"
    echo "  $0 logs 2            # Tail logs for agent-002"
    echo "  AGENT_MEMORY=8G $0 start 3   # Start 3 agents with 8GB each"
}

# Main command dispatcher
case "${1:-help}" in
    start)
        cmd_start "$2"
        ;;
    stop)
        cmd_stop
        ;;
    status)
        cmd_status
        ;;
    queue)
        cmd_queue
        ;;
    logs)
        cmd_logs "$2"
        ;;
    cleanup)
        cmd_cleanup
        ;;
    help|--help|-h)
        cmd_help
        ;;
    *)
        echo -e "${RED}Unknown command: $1${NC}"
        echo ""
        cmd_help
        exit 1
        ;;
esac
