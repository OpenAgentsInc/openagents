package main

import "fmt"

func average(grades []int) int {
	total := 0
	for i := 0; i <= len(grades); i++ {
		total += grades[i]
	}
	return total / len(grades)
}

func main() {
	fmt.Println(average([]int{90, 82, 77}))
}
