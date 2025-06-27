// Mock deployment simulator for testing
import type { DeploymentStatus } from './types';

export class MockDeploymentSimulator {
  private stages = [
    { stage: 'Initializing', progress: 0, message: 'Preparing deployment environment' },
    { stage: 'Cloning repository', progress: 10, message: 'Fetching source code from repository' },
    { stage: 'Installing dependencies', progress: 20, message: 'Running npm install' },
    { stage: 'Building project', progress: 40, message: 'Compiling TypeScript and bundling assets' },
    { stage: 'Running tests', progress: 50, message: 'Executing test suite' },
    { stage: 'Optimizing bundle', progress: 60, message: 'Minimizing and optimizing for production' },
    { stage: 'Creating container', progress: 70, message: 'Building deployment container' },
    { stage: 'Deploying to edge', progress: 80, message: 'Uploading to Cloudflare network' },
    { stage: 'Configuring DNS', progress: 90, message: 'Setting up domain and SSL certificates' },
    { stage: 'Finalizing', progress: 95, message: 'Running health checks' },
    { stage: 'Complete', progress: 100, message: 'Deployment successful!' }
  ];

  async simulateDeployment(
    deploymentId: string,
    projectName: string,
    onUpdate: (status: DeploymentStatus) => Promise<void>
  ) {
    console.log(`Starting mock deployment simulation for ${deploymentId}`);
    
    for (let i = 0; i < this.stages.length; i++) {
      const stage = this.stages[i];
      const isLast = i === this.stages.length - 1;
      
      const status: DeploymentStatus = {
        id: deploymentId,
        projectId: projectName,
        status: isLast ? 'success' : (i < 3 ? 'building' : 'deploying'),
        progress: stage.progress,
        stage: stage.stage,
        message: stage.message,
        timestamp: Date.now(),
        logs: [`[${new Date().toISOString()}] ${stage.message}`]
      };

      // Add deployment URL on completion
      if (isLast) {
        status.deploymentUrl = `https://${projectName.toLowerCase().replace(/\s+/g, '-')}-${deploymentId.slice(0, 8)}.openagents.dev`;
      }

      // Simulate some failures occasionally
      if (Math.random() < 0.1 && i > 3 && i < 9) {
        status.status = 'error';
        status.message = `Failed during ${stage.stage}: Simulated error for testing`;
        await onUpdate(status);
        console.log(`Deployment ${deploymentId} failed at ${stage.stage}`);
        return;
      }

      await onUpdate(status);
      
      // Random delay between stages (1-3 seconds)
      const delay = 1000 + Math.random() * 2000;
      await new Promise(resolve => setTimeout(resolve, delay));
    }
    
    console.log(`Deployment ${deploymentId} completed successfully`);
  }
}