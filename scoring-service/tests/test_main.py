from main import main

def test_main(capsys):
    """Test that main function prints expected output."""
    main()
    captured = capsys.readouterr()
    assert captured.out == "Hello from scoring-service!\n"

