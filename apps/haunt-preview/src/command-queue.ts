/**
 * A small FIFO promise queue with a fixed capacity.
 *
 * Capacity counts jobs that are running or waiting. Jobs run one at a time
 * in submission order. Every caller receives the result or error of its own
 * job, and a job that rejects (or throws synchronously) still releases its
 * capacity so later queued jobs can run.
 */
export class CommandQueue {
  static readonly limit = 32;

  private pending = 0;
  private active = false;
  private readonly waiting: Array<() => void> = [];

  /**
   * Run `job` after all previously submitted jobs have settled.
   *
   * Rejects immediately with a stable message when 32 jobs are already
   * running or waiting. The returned promise settles with this job's own
   * result or error; it never observes another job's outcome.
   */
  run<T>(job: () => Promise<T>): Promise<T> {
    if (this.pending >= CommandQueue.limit) {
      return Promise.reject(
        new Error("Wait for the current changes to finish."),
      );
    }

    this.pending += 1;

    return new Promise<T>((resolve, reject) => {
      const start = () => {
        this.active = true;
        let result: Promise<T>;
        try {
          result = job();
        } catch (error) {
          this.settle();
          reject(error);
          return;
        }
        Promise.resolve(result).then(
          (value) => {
            this.settle();
            resolve(value);
          },
          (error) => {
            this.settle();
            reject(error);
          },
        );
      };

      if (this.active || this.waiting.length > 0) {
        this.waiting.push(start);
      } else {
        start();
      }
    });
  }

  /** Release one slot and start the next waiting job, if any. */
  private settle() {
    this.pending -= 1;
    this.active = false;
    const next = this.waiting.shift();
    if (next) next();
  }
}
