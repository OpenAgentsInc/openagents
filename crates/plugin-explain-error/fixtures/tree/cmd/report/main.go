package main

import "fmt"

func average(scores []int) int {
	total := 0
	for i := 0; i <= len(scores); i++ {
		total += scores[i]
	}
	return total / len(scores)
}

func main() {
	fmt.Println(average([]int{90, 82, 77}))
}
