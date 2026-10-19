package main

import (
	"encoding/json"
	"fmt"
	knot "knot.local/bindings"
	"os"
)

func run() error {
	if len(os.Args) != 3 {
		return fmt.Errorf("usage: smoke <speaker.json> <speaker-events.json>")
	}
	product, err := os.ReadFile(os.Args[1])
	if err != nil {
		return err
	}
	eventsRaw, err := os.ReadFile(os.Args[2])
	if err != nil {
		return err
	}
	var events []json.RawMessage
	if err = json.Unmarshal(eventsRaw, &events); err != nil {
		return err
	}
	model, _, err := knot.Compile(product)
	if err != nil {
		return err
	}
	defer model.Close()
	raw, err := model.Initial()
	if err != nil {
		return err
	}
	var result struct {
		State    json.RawMessage `json:"state"`
		Snapshot json.RawMessage `json:"snapshot"`
	}
	for _, event := range events {
		if err = json.Unmarshal(raw, &result); err != nil {
			return err
		}
		raw, err = model.Apply(result.State, event)
		if err != nil {
			return err
		}
		fmt.Println(string(raw))
	}
	if err = json.Unmarshal(raw, &result); err != nil {
		return err
	}
	_, err = model.Evaluate(result.State)
	return err
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
