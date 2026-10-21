/* ЛБ2, задача 3: do-while, switch, continue.
 * Меню-калькулятор: команда, два числа, результат. */
#include <stdio.h>

#define CMD_ADD 1
#define CMD_SUB 2
#define CMD_MUL 3
#define CMD_EXIT 4

int main(void) {
    int cmd = 0;
    double a = 0.0;
    double b = 0.0;
    double res = 0.0;

    do {
        printf("1 - сложить, 2 - вычесть, 3 - умножить, 4 - выход: ");
        if (scanf("%d", &cmd) != 1) {
            printf("нужна цифра команды\n");
            while (getchar() != '\n') {
            }
            continue;
        }
        if (cmd == CMD_EXIT) {
            break;
        }
        if (cmd < CMD_ADD || cmd > CMD_MUL) {
            printf("нет такой команды\n");
            continue;
        }
        printf("два числа: ");
        scanf("%lf %lf", &a, &b);
        switch (cmd) {
            case CMD_ADD:
                res = a + b;
                break;
            case CMD_SUB:
                res = a - b;
                break;
            case CMD_MUL:
                res = a * b;
                break;
            default:
                res = 0.0;
                break;
        }
        printf("результат = %.3lf\n", res);
    } while (1);
    printf("работа завершена\n");
    return 0;
}
