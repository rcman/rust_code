This Rust example showcases several important language features:
Key Rust Concepts Demonstrated:

Structs and Methods: The Student struct with associated methods
Ownership and Borrowing: The print_student_info function takes a reference (&Student) instead of taking ownership
Pattern Matching: Used in the letter_grade method with match expressions
Option Types: Handling cases where values might not exist (no grades recorded)
Error Handling: Validating grade inputs and using Option for safe operations
Collections: Using HashMap and Vec for data storage
Iterators: Using iterator methods like sum() and max_by()
Memory Safety: Rust's ownership system prevents common bugs like null pointer dereferences

The code creates a simple grade management system where you can add students, record their grades, calculate averages, and find the top performer. Run it with cargo run after setting up a new Rust project with cargo new grade_manager.
