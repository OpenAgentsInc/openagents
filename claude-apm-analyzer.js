#!/usr/bin/env node

/**
 * Claude Code APM Analyzer
 * 
 * Analyzes existing Claude Code conversations to calculate Actions Per Minute (APM) metrics
 * Based on StarCraft APM concepts and Claude Code's JSONL conversation format
 */

import fs from 'fs';
import path from 'path';
import { homedir } from 'os';

const CLAUDE_DIR = path.join(homedir(), '.claude');
const PROJECTS_DIR = path.join(CLAUDE_DIR, 'projects');

class ClaudeAPMAnalyzer {
  constructor() {
    this.conversations = [];
    this.stats = {
      totalSessions: 0,
      totalMessages: 0,
      totalToolUses: 0,
      totalDuration: 0,
      toolFrequency: {},
      sessionStats: []
    };
  }

  async analyze() {
    console.log('🔍 Claude Code APM Analyzer\n');
    
    if (!fs.existsSync(PROJECTS_DIR)) {
      console.error(`❌ Claude projects directory not found: ${PROJECTS_DIR}`);
      console.error('Make sure you have used Claude Code before running this analyzer.');
      process.exit(1);
    }

    console.log(`📁 Scanning: ${PROJECTS_DIR}`);
    this.loadConversations();
    this.calculateMetrics();
    this.displayResults();
  }

  loadConversations() {
    const projectDirs = fs.readdirSync(PROJECTS_DIR)
      .filter(item => fs.statSync(path.join(PROJECTS_DIR, item)).isDirectory());

    console.log(`📊 Found ${projectDirs.length} project directories`);

    for (const projectDir of projectDirs) {
      const projectPath = path.join(PROJECTS_DIR, projectDir);
      const jsonlFiles = fs.readdirSync(projectPath)
        .filter(file => file.endsWith('.jsonl'));

      for (const file of jsonlFiles) {
        const filePath = path.join(projectPath, file);
        this.parseConversation(filePath, projectDir);
      }
    }

    console.log(`✅ Loaded ${this.conversations.length} conversations\n`);
  }

  parseConversation(filePath, projectName) {
    try {
      const content = fs.readFileSync(filePath, 'utf8');
      const lines = content.split('\n').filter(line => line.trim());
      
      if (lines.length === 0) return;

      const messages = [];
      const toolUses = [];
      let startTime = null;
      let endTime = null;
      let sessionId = null;

      for (const line of lines) {
        try {
          const entry = JSON.parse(line);
          
          // Extract session metadata
          if (entry.sessionId && !sessionId) {
            sessionId = entry.sessionId;
          }

          // Parse timestamps
          if (entry.timestamp) {
            const timestamp = new Date(entry.timestamp);
            if (!startTime || timestamp < startTime) startTime = timestamp;
            if (!endTime || timestamp > endTime) endTime = timestamp;
          }

          // Count messages
          if (entry.type === 'user' || entry.type === 'assistant') {
            messages.push({
              type: entry.type,
              timestamp: entry.timestamp ? new Date(entry.timestamp) : null,
              content: entry.message?.content || ''
            });
          }

          // Extract tool usage
          if (entry.type === 'assistant' && entry.message?.content) {
            const content = entry.message.content;
            if (Array.isArray(content)) {
              for (const item of content) {
                if (item.type === 'tool_use') {
                  toolUses.push({
                    name: item.name,
                    timestamp: entry.timestamp ? new Date(entry.timestamp) : null,
                    input: item.input || {}
                  });
                }
              }
            }
          }
        } catch (parseError) {
          // Skip invalid JSON lines
          continue;
        }
      }

      if (messages.length > 0 && startTime && endTime) {
        const conversation = {
          filePath,
          projectName: this.cleanProjectName(projectName),
          sessionId: sessionId || path.basename(filePath, '.jsonl'),
          startTime,
          endTime,
          duration: (endTime - startTime) / 1000, // seconds
          messageCount: messages.length,
          toolUses,
          messages
        };

        this.conversations.push(conversation);
      }
    } catch (error) {
      console.warn(`⚠️  Failed to parse ${filePath}: ${error.message}`);
    }
  }

  cleanProjectName(projectName) {
    return projectName
      .replace(/^-Users-[^-]+-/, '~/')
      .replace(/-/g, '/');
  }

  calculateMetrics() {
    this.stats.totalSessions = this.conversations.length;
    
    for (const conv of this.conversations) {
      this.stats.totalMessages += conv.messageCount;
      this.stats.totalToolUses += conv.toolUses.length;
      this.stats.totalDuration += conv.duration;

      // Count tool frequency
      for (const tool of conv.toolUses) {
        this.stats.toolFrequency[tool.name] = (this.stats.toolFrequency[tool.name] || 0) + 1;
      }

      // Calculate session-specific APM
      const sessionAPM = this.calculateSessionAPM(conv);
      this.stats.sessionStats.push(sessionAPM);
    }

    // Sort sessions by APM
    this.stats.sessionStats.sort((a, b) => b.totalAPM - a.totalAPM);
  }

  calculateSessionAPM(conversation) {
    const durationMinutes = conversation.duration / 60;
    if (durationMinutes <= 0) return null;

    const messageAPM = conversation.messageCount / durationMinutes;
    const toolAPM = conversation.toolUses.length / durationMinutes;
    const totalActions = conversation.messageCount + conversation.toolUses.length;
    const totalAPM = totalActions / durationMinutes;

    // Categorize tool types
    const toolCategories = {
      codeGeneration: ['Edit', 'MultiEdit', 'Write'],
      fileOperations: ['Read', 'LS', 'Glob'],
      systemOperations: ['Bash'],
      search: ['Grep', 'WebSearch', 'WebFetch'],
      planning: ['TodoWrite'],
      other: []
    };

    const toolStats = {};
    for (const [category, tools] of Object.entries(toolCategories)) {
      toolStats[category] = conversation.toolUses.filter(tool => 
        tools.includes(tool.name)
      ).length / durationMinutes;
    }

    return {
      sessionId: conversation.sessionId,
      projectName: conversation.projectName,
      duration: conversation.duration,
      durationMinutes,
      messageCount: conversation.messageCount,
      toolCount: conversation.toolUses.length,
      messageAPM,
      toolAPM,
      totalAPM,
      toolStats,
      startTime: conversation.startTime,
      endTime: conversation.endTime
    };
  }

  displayResults() {
    console.log('📈 CLAUDE CODE APM ANALYSIS RESULTS');
    console.log('=' .repeat(50));
    
    // Overall stats
    const totalMinutes = this.stats.totalDuration / 60;
    const overallAPM = totalMinutes > 0 ? 
      (this.stats.totalMessages + this.stats.totalToolUses) / totalMinutes : 0;

    console.log('\n🌍 OVERALL STATISTICS');
    console.log(`Total Sessions: ${this.stats.totalSessions}`);
    console.log(`Total Messages: ${this.stats.totalMessages}`);
    console.log(`Total Tool Uses: ${this.stats.totalToolUses}`);
    console.log(`Total Duration: ${(totalMinutes / 60).toFixed(1)} hours`);
    console.log(`Overall APM: ${overallAPM.toFixed(1)} actions/minute`);

    // Top tools
    console.log('\n🛠️  TOP TOOLS BY FREQUENCY');
    const sortedTools = Object.entries(this.stats.toolFrequency)
      .sort(([,a], [,b]) => b - a)
      .slice(0, 10);
    
    for (const [tool, count] of sortedTools) {
      const percentage = ((count / this.stats.totalToolUses) * 100).toFixed(1);
      console.log(`${tool.padEnd(15)} ${count.toString().padStart(4)} uses (${percentage}%)`);
    }

    // APM ranges (like StarCraft skill tiers)
    console.log('\n🎯 APM SKILL TIERS');
    const apmRanges = [
      { name: 'Novice', min: 0, max: 10, color: '🟤' },
      { name: 'Casual', min: 10, max: 25, color: '🟢' },
      { name: 'Active', min: 25, max: 50, color: '🟡' },
      { name: 'Productive', min: 50, max: 100, color: '🟠' },
      { name: 'Professional', min: 100, max: 200, color: '🔴' },
      { name: 'Elite', min: 200, max: Infinity, color: '🟣' }
    ];

    for (const range of apmRanges) {
      const sessionsInRange = this.stats.sessionStats.filter(s => 
        s.totalAPM >= range.min && s.totalAPM < range.max
      ).length;
      const percentage = this.stats.totalSessions > 0 ? 
        ((sessionsInRange / this.stats.totalSessions) * 100).toFixed(1) : 0;
      console.log(`${range.color} ${range.name.padEnd(12)} ${range.min}-${range.max === Infinity ? '∞' : range.max} APM: ${sessionsInRange} sessions (${percentage}%)`);
    }

    // Top sessions
    console.log('\n🏆 TOP 10 HIGHEST APM SESSIONS');
    console.log('Rank | APM   | Duration | Messages | Tools | Project');
    console.log('-'.repeat(60));
    
    const topSessions = this.stats.sessionStats.slice(0, 10);
    topSessions.forEach((session, index) => {
      const rank = (index + 1).toString().padStart(2);
      const apm = session.totalAPM.toFixed(1).padStart(5);
      const duration = `${(session.durationMinutes).toFixed(1)}m`.padStart(8);
      const messages = session.messageCount.toString().padStart(8);
      const tools = session.toolCount.toString().padStart(5);
      const project = session.projectName.length > 20 ? 
        session.projectName.substring(0, 17) + '...' : session.projectName;
      
      console.log(`${rank}   | ${apm} | ${duration} | ${messages} | ${tools} | ${project}`);
    });

    // Time-based analysis
    console.log('\n⏰ PRODUCTIVITY BY TIME PERIOD');
    this.analyzeProductivityByTime();

    console.log('\n📊 Use --detailed flag for individual session breakdowns');
    console.log('💡 Tip: Aim for 50+ APM for productive coding sessions');
  }

  analyzeProductivityByTime() {
    const timeSlots = {
      'Morning (6-12)': [],
      'Afternoon (12-18)': [],
      'Evening (18-24)': [],
      'Night (0-6)': []
    };

    for (const session of this.stats.sessionStats) {
      const hour = session.startTime.getHours();
      if (hour >= 6 && hour < 12) timeSlots['Morning (6-12)'].push(session);
      else if (hour >= 12 && hour < 18) timeSlots['Afternoon (12-18)'].push(session);
      else if (hour >= 18 && hour < 24) timeSlots['Evening (18-24)'].push(session);
      else timeSlots['Night (0-6)'].push(session);
    }

    for (const [period, sessions] of Object.entries(timeSlots)) {
      if (sessions.length === 0) continue;
      
      const avgAPM = sessions.reduce((sum, s) => sum + s.totalAPM, 0) / sessions.length;
      const totalSessions = sessions.length;
      
      console.log(`${period.padEnd(18)} ${avgAPM.toFixed(1)} avg APM (${totalSessions} sessions)`);
    }
  }
}

// CLI argument parsing
const args = process.argv.slice(2);
const detailed = args.includes('--detailed') || args.includes('-d');

// Run analyzer
const analyzer = new ClaudeAPMAnalyzer();
analyzer.analyze().catch(console.error);